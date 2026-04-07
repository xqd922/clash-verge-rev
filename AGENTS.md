# Repository Guidelines

## Project Structure & Module Organization
`src/` contains the React 19 + Vite frontend. Keep UI in `components/`, route-level screens in `pages/`, shared logic in `hooks/`, `providers/`, `services/`, and utility helpers in `utils/`. Localized strings live in `src/locales/`, and generated TS types live under `src/types/generated/`.

`src-tauri/` contains the desktop shell, bundled resources, and the main Rust application. Shared Rust crates live in `crates/` (for example `clash-verge-i18n`, `clash-verge-logging`, and `tauri-plugin-mihomo`). Build and release helpers are in `scripts/`, with additional workflow utilities in `scripts-workflow/`. Project docs are under `docs/`.

## Build, Test, and Development Commands
Use `pnpm` for JS tasks and `cargo` for Rust tasks.

- `pnpm dev`: start the default Tauri dev app.
- `pnpm dev:tauri`: run the pure Tauri development profile.
- `pnpm web:dev`: run the frontend only in Vite.
- `pnpm run prebuild`: download or refresh Mihomo sidecar binaries.
- `pnpm build` / `pnpm build:fast`: create production or faster test builds.
- `pnpm lint` and `pnpm typecheck`: run frontend linting and TS checks.
- `cargo test --workspace`: run Rust tests across workspace crates.
- `cargo test -p tauri-plugin-mihomo`: run the plugin integration tests in `crates/tauri-plugin-mihomo/tests/`.
- `cargo make pre-commit` / `cargo make pre-push`: run the same checks wired into Husky hooks.

## Coding Style & Naming Conventions
Follow `.editorconfig`: 2 spaces for TS/JS/JSON/Markdown, 4 spaces for Rust, LF endings, UTF-8. Prettier enforces 80-column formatting, semicolons, double quotes, and trailing commas. Rust uses `rustfmt` with a 120-column limit.

Prefer `kebab-case` for frontend file names (`traffic-sampler.ts`), `PascalCase` for React components, and clear crate names matching the existing `clash-verge-*` pattern. Run `pnpm format` and `cargo fmt` before pushing.

## Testing Guidelines
Frontend changes must pass `pnpm lint` and `pnpm typecheck`. Rust changes should pass `cargo clippy --all-targets --all-features -- -D warnings` and relevant `cargo test` targets. Add Rust tests in crate-level `tests/` folders using descriptive `*_test.rs` names. There is no dedicated JS unit-test runner configured, so validate UI changes in `pnpm dev` and include screenshots for visible changes.

## Commit & Pull Request Guidelines
Recent history uses concise conventional prefixes such as `fix:`, `perf:`, and `release:`. Keep commit subjects imperative and scoped to one change. Signed commits are required.

Pull requests should describe the change, note affected platforms, link related issues, and include screenshots or recordings for UI work. Call out any sidecar, packaging, or migration impact explicitly.
