# Repository Guidelines

## Project Structure & Module Organization

- `src/`: React 19 + TypeScript frontend (pages, components, hooks, providers, services, locales, assets).
- `src-tauri/`: Rust backend for the Tauri 2 app (IPC commands in `src-tauri/src/cmd/`, core services in `src-tauri/src/core/`, config/enhance pipeline in `src-tauri/src/config/` and `src-tauri/src/enhance/`).
- `crates/`: Rust workspace crates (logging, i18n, limiter, sysinfo plugin, etc.).
- `scripts/`: Node scripts (notably `scripts/prebuild.mjs` for downloading/packaging the mihomo sidecar binaries).
- `dist/`: Vite build output (Tauri bundles this via `src-tauri/tauri.conf.json`).

## Build, Test, and Development Commands

- `pnpm install`: install frontend tooling and script deps.
- `pnpm run prebuild`: download/prepare mihomo core binaries required for local run.
- `pnpm dev`: run Tauri dev (backend + frontend). Use `pnpm web:dev` for frontend-only.
- `pnpm build` / `pnpm build:fast`: production build (fast uses Rust `fast-release` profile).
- `pnpm lint` / `pnpm typecheck` / `pnpm format`: ESLint, TypeScript checks, Prettier formatting.
- `cargo fmt` / `cargo clippy --all-targets --all-features`: Rust formatting and linting.
- `cargo test`: run Rust unit tests (some exist under `src-tauri/src/**`).

## Coding Style & Naming Conventions

- TypeScript/React: 2-space indentation; prefer `PascalCase` components and `camelCase` functions/vars. Keep UI logic in `src/pages/` and shared logic in `src/services/`, `src/hooks/`.
- Rust: `snake_case` for modules/functions, `CamelCase` for types; avoid `unwrap()`/`expect()` (workspace Clippy lints are strict).
- Run formatters before PR: `pnpm format` and `cargo fmt`.

## Testing Guidelines

- Rust: add focused unit tests near the module under test (`#[test]`); keep tests deterministic.
- Frontend: no dedicated test runner is enforced; prefer small, testable utilities and manual verification steps in PR notes.

## Commit & Pull Request Guidelines

- Commits generally follow Conventional Commit style: `feat:`, `fix:`, `refactor:`, `chore:`, `perf:`, `doc:`, `style:`, `ci:` with optional scopes (e.g., `fix(macos): ...`).
- PRs: describe the problem + approach, link related issues, include screenshots/GIFs for UI changes, and note platform-specific behavior (Windows/macOS/Linux) when relevant.

## Security & Configuration Tips

- Treat Tauri capabilities (`src-tauri/capabilities/`) as security boundaries; avoid widening `fs`/`shell` scopes without strong justification.
- For profile scripts (Boa JS in `src-tauri/src/enhance/`), consider input size and execution time impact when adding new features.
