# Legacy Release Pipeline Design

Date: 2026-05-24
Branch: `recovery/legacy-clean-2026-05-24`

## Goal

Restore a conservative 1.x legacy release path without replaying the risky history from `release/1.x-legacy`.

## Approved Direction

Use the conservative rewrite approach:

- Keep the legacy product/release concept.
- Restore local build hardening and legacy resource preparation.
- Restore release helper scripts only where they are locally testable.
- Add a dedicated `release-1x-legacy.yml` workflow with scoped permissions.
- Do not default to release/tag deletion.
- Do not replay runtime behavior changes such as warm-to-tray, profile switching rewrites, global SWR focus changes, or port/default tuning.

## Alternatives Considered

### Direct Cherry-Pick

Fast, but rejected for this recovery branch. The old workflow used `permissions: write-all`, defaulted `overwrite_existing` to `true`, and included a delete-release/delete-tag path. Several follow-up CI commits and a revert show the pipeline was unstable.

### No Release Pipeline

Safest for runtime behavior, but leaves the recovery branch unable to produce legacy artifacts. That does not satisfy the cleanup goal because release mechanics are part of the legacy branch.

### Conservative Rewrite

Chosen approach. It preserves the useful pieces while removing destructive defaults and keeping each local piece verifiable.

## Design

### Local Build Wrapper

`pnpm build` should run `pnpm check` before `pnpm tauri build`. This prevents packaging with missing sidecars/resources. The wrapper should also set a Node heap limit unless one is already present.

Files:

- `scripts/build.mjs`
- `package.json`
- `README.md`
- `CONTRIBUTING.md`

### Resource Preparation

`scripts/check.mjs` should be hardened so downloads fail on HTTP errors, HTML error pages, empty bodies, or invalid version payloads. Windows legacy builds should use the `v1.7.7` portable package for the service binaries by default.

Files:

- `scripts/check.mjs`

### Cargo Build Guard

`src-tauri/build.rs` should fail early if required runtime artifacts are missing. This catches direct `pnpm tauri build` usage without a prior `pnpm check`.

Files:

- `src-tauri/build.rs`

### Legacy Release Metadata

`scripts/prepare-legacy-release.mjs` should rewrite package/product identifiers for legacy packaging. It should require a tag containing `-legacy.` and should only modify known package/config files.

Files:

- `scripts/prepare-legacy-release.mjs`
- `package.json`

### Legacy Portable and Updater Scripts

Add legacy-specific scripts instead of overloading the mainline updater/portable scripts. These scripts should require `GITHUB_TOKEN` for upload paths and should target the legacy product names and updater release assets.

Files:

- `scripts/portable-legacy.mjs`
- `scripts/portable-fixed-webview2-legacy.mjs`
- `scripts/updater-legacy.mjs`
- `scripts/updater-fixed-webview2-legacy.mjs`
- `scripts/print-updatelog.mjs`
- `package.json`

### Dedicated Workflow

Add `.github/workflows/release-1x-legacy.yml`.

Rules:

- `workflow_dispatch` only.
- `overwrite_existing` default is `false`.
- `permissions` are scoped to `contents: write`.
- Release deletion is allowed only when manually requested.
- Windows-only legacy matrix, matching the original legacy intent.
- `LEGACY_SERVICE_TAG` is pinned to `v1.7.7`.
- Node heap is `8192` for Tauri builds.

## Verification

Required local gates:

- `pnpm install --frozen-lockfile --prefer-offline`
- `pnpm check x86_64-pc-windows-msvc`
- `node scripts/prepare-legacy-release.mjs v1.7.7-legacy.99` in a disposable copy or with reverted generated file changes afterward.
- `$env:NODE_OPTIONS='--max_old_space_size=8192'; pnpm web:build`
- `cargo test` in `src-tauri`

Workflow validation is structural in this environment:

- inspect YAML for `permissions: contents: write`;
- inspect YAML for `overwrite_existing default: false`;
- inspect YAML for absence of unconditional release/tag deletion.

## Explicit Non-Goals

- No warm-to-tray or close-to-tray changes.
- No profile switch flow rewrite.
- No global SWR focus policy changes.
- No port/default tuning.
- No dependency/schema bump in this phase.
