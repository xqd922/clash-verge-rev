# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Clash Verge Rev — a Tauri 1.x desktop GUI for the Mihomo (Clash Meta) proxy core. Frontend is React 18 + TypeScript + MUI 5 + Vite; backend is Rust under `src-tauri/`. Package manager is **pnpm 9.1.4** (use `pnpm`, not `npm`/`yarn`).

## Commands

```bash
pnpm i                  # install JS deps
pnpm run check          # download the mihomo sidecar binary into src-tauri/sidecar/
pnpm run check --force  # force re-download latest mihomo
pnpm dev                # tauri dev (full app)
pnpm dev:diff           # tauri dev with `verge-dev` cargo feature — use when a normal dev instance is already running so the two won't collide on the singleton port
pnpm build              # tauri build (production bundle)
pnpm web:dev            # frontend only (Vite at :3000), used by tauri.conf.json beforeDevCommand
pnpm web:build          # tsc + vite build → ../dist
```

`pnpm run check` is a **mandatory first step on a fresh clone** — without it the mihomo sidecar is missing and `tauri dev` will fail to spawn the core. The script in `scripts/check.mjs` resolves the host triple via `rustc -vV` and downloads platform-matched binaries to `src-tauri/sidecar/verge-mihomo[-alpha]-<triple>[.exe]`.

There is **no test suite and no linter command**. Quality gate is `pnpm pretty-quick --staged` via the husky pre-commit hook. TypeScript strict mode is on, so `pnpm web:build` doubles as a type check.

Other scripts in `scripts/`: `updater.mjs`, `updater-fixed-webview2.mjs`, `portable.mjs`, `portable-fixed-webview2.mjs`, `updatelog.mjs` — release/packaging tooling, generally only invoked from CI.

## Architecture

### IPC contract: frontend ↔ Rust

Frontend calls Rust via `@tauri-apps/api` `invoke()`. The contract spans three files that **must stay in sync**:

1. `src-tauri/src/cmds.rs` — `#[tauri::command]` functions.
2. `src-tauri/src/main.rs` — every command registered in the `tauri::generate_handler![...]` block. **Adding a command requires editing this list.**
3. `src/services/cmds.ts` — thin TypeScript wrappers around `invoke<T>("cmd_name", args)`. TS types live in `src/services/types.d.ts`.

When adding a command, all three sites must be updated together or the call will fail at runtime with a "command not found" error.

### Rust backend (`src-tauri/src/`)

- `main.rs` — Tauri entry. Performs singleton check (`utils::server::check_singleton`), then builds the app with system tray + all command handlers. Window lifecycle events persist position/size via `resolve::save_window_size_position`.
- `cmds.rs` — All `#[tauri::command]` handlers; the IPC surface.
- `feat.rs` — Higher-level operations (toggle TUN, change core, etc.) called by both `cmds.rs` and the tray.
- `config/` — Layered config state. **The central pattern is `Config::clash() / verge() / profiles() / runtime()` returning a `Draft<T>`** where `.latest()` reads the current in-memory state and `.draft()` / `.apply()` / `.discard()` implement copy-on-write edits before persisting. `prfitem.rs` models a single profile; `runtime.rs` holds the merged runtime config after enhancement.
- `core/` — Long-running subsystems:
  - `core.rs` — manages the mihomo sidecar process (spawn/kill/restart) and pushes config to it.
  - `clash_api.rs` — HTTP client against the mihomo external controller.
  - `tray.rs` — system tray menu + event handling.
  - `sysopt.rs` — system proxy set/guard and auto-launch.
  - `handle.rs` — global access to the Tauri `AppHandle` / main window.
  - `hotkey.rs`, `timer.rs` (`delay_timer` scheduled jobs), `service.rs` (privileged service-mode installer), `logger.rs`, `win_uwp.rs`.
- `enhance/` — Profile enhancement pipeline. `enhance()` in `mod.rs` is the entry; it composes `merge` (YAML merge layers), `script` (JS via `boa_engine`), `seq` (sequence merges), `chain` (ordered application of items), `field` (filter to known clash fields), `tun` (TUN-mode overrides), and `builtin/` scripts. The output is the runtime config fed to mihomo.
- `utils/` — `dirs.rs` (XDG-ish app dirs, portable-mode detection), `init.rs` (first-run config bootstrap), `resolve.rs` (window/setup lifecycle), `server.rs` (warp-based singleton check + deep-link handler), `tmpl.rs` (embedded config templates), `help.rs`.

### Frontend (`src/`)

- Entry: `main.tsx` → `pages/_layout.tsx` (shell) + `pages/_routers.tsx` (routes) + `pages/_theme.tsx` (MUI theme). Pages: `proxies`, `profiles`, `rules`, `connections`, `logs`, `settings`, `test`.
- `services/` — `cmds.ts` (Tauri invokes), `api.ts` (mihomo HTTP/WS client), `delay.ts` (latency tester), `i18n.ts`, `states.ts` (jotai-style atoms), `types.d.ts` (shared types, **declared globally — no imports needed**).
- `hooks/` — SWR-backed hooks: `use-clash`, `use-profiles`, `use-verge`, `use-log-data`, `use-visibility`.
- `components/` — grouped by feature: `base`, `connection`, `layout`, `log`, `profile`, `proxy`, `rule`, `setting`, `test`.
- Path aliases: `@/*` → `src/*`, `@root/*` → repo root (see `tsconfig.json` and `vite.config.ts`).
- Vite `root` is `src/` (so `src/index.html` is the entry) and output goes to `../dist`, matching `tauri.conf.json` `distDir`. The build is bundled with Monaco editor workers (YAML/TS/CSS) and legacy targets edge ≥109 / safari ≥13.

### Tauri configuration

`tauri.conf.json` is the base; **per-platform overrides** live in `tauri.linux.conf.json`, `tauri.macos.conf.json`, `tauri.windows.conf.json` and are merged by Tauri at build time. The sidecar (`externalBin`) is `sidecar/verge-mihomo` plus `sidecar/verge-mihomo-alpha`; both must be present after `pnpm run check`.

The `verge-dev` cargo feature (declared in `Cargo.toml`) is what makes `pnpm dev:diff` produce an isolated dev instance that won't trip the singleton lock against a running release build.

### Conventions worth knowing

- Mixed Chinese / English comments and identifiers are expected — match the surrounding style of the file you're editing.
- Profile enhancement is the conceptual heart of the app: a user's raw subscription YAML is layered with their merge/script/tun overrides before being handed to mihomo. When changing config behavior, trace through `enhance::enhance()` to make sure the layering still composes correctly.
- Release profile uses `lto = true`, `codegen-units = 1`, `opt-level = "s"`, `panic = "abort"` — release builds are slow; rely on `pnpm dev` for iteration.

## Release process

This is a fork. Releases follow a strict convention enforced by `.github/workflows/release.yml`:

- **Versioning**: `<upstream-base>-legacy.r<N>`, e.g. `1.7.7-legacy.r1` → `1.7.7-legacy.r2`. The upstream base (`1.7.7`) only bumps when we pull a new upstream tag; `rN` increments on every fork build. Tags with no suffix (`v1.7.8`) are marked stable; anything with a `-legacy` / `-rc` / `-beta` / `-alpha` suffix is auto-marked prerelease.
- **Three sources of truth must match**: `package.json` `version`, `src-tauri/tauri.conf.json` `package.version`, and the git tag (sans leading `v`). The workflow's `preflight` job fails the run if they disagree.
- **UPDATELOG.md is required for every release**. The release-notes script tries the full tag first (e.g. `v1.7.7-legacy.r2`) and falls back to the upstream base section (`v1.7.7`) if there is no fork-specific entry. Add a fork-specific section only when the `rN` build has notes worth differentiating; otherwise the base section is reused automatically.
- **Cutting a release**:
  1. Bump version in `package.json` and `src-tauri/tauri.conf.json` (keep them identical).
  2. Add or update the matching section in `UPDATELOG.md` if needed.
  3. Commit, then `git tag v<version> && git push && git push --tags`.
  4. The `Release Build` workflow triggers on `v*` tag push: `preflight` validates and creates a draft release with UPDATELOG body → matrix builds upload assets → `publish-release` flips draft to published → updater.json refresh.
- **Manual dispatch**: the workflow also accepts `workflow_dispatch` with a `tag` input, but the tag must already be pushed (the checkout step fails otherwise). Use this only for re-runs.
