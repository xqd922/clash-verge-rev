# Rebuild 1.x Legacy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild `release/1.x-legacy` from the clean `release/1.7.x-legacy` baseline while preserving the useful behavior from `origin/release/1.x-legacy`.

**Architecture:** Treat `origin/release/1.x-legacy` as a patch source, not as the branch to continue. Recreate the branch as focused commits by migration area: release packaging, backend identity/configuration, window/tray behavior, frontend fixes, and documentation.

**Tech Stack:** Git, PowerShell, pnpm 9.1.4, Vite/React/TypeScript, Rust/Tauri 1.x, GitHub Actions.

---

## File Structure

The implementation must happen in an isolated worktree rooted at
`../clash-verge-rev-rebuild-1x-legacy`, not in the current working directory.
The current directory has unrelated local changes:

- `.github/workflows/release.yml`
- `src-tauri/Cargo.toml`
- `release_notes.local.md`

Those files must not be overwritten or included by accident.

The rebuild branch will modify these groups:

- Release workflow and scripts:
  - `.github/workflows/release-1x-legacy.yml`
  - `package.json`
  - `pnpm-lock.yaml`
  - `scripts/build.mjs`
  - `scripts/check.mjs`
  - `scripts/prepare-legacy-release.mjs`
  - `scripts/portable-legacy.mjs`
  - `scripts/portable-fixed-webview2-legacy.mjs`
  - `scripts/updater-legacy.mjs`
  - `scripts/updater-fixed-webview2-legacy.mjs`
  - `scripts/print-updatelog.mjs`
  - `scripts/updatelog.mjs`
- Tauri backend and runtime behavior:
  - `src-tauri/Cargo.toml`
  - `src-tauri/Cargo.lock`
  - `src-tauri/build.rs`
  - `src-tauri/src/cmds.rs`
  - `src-tauri/src/config/clash.rs`
  - `src-tauri/src/config/verge.rs`
  - `src-tauri/src/core/clash_api.rs`
  - `src-tauri/src/core/core.rs`
  - `src-tauri/src/core/sysopt.rs`
  - `src-tauri/src/core/tray.rs`
  - `src-tauri/src/feat.rs`
  - `src-tauri/src/main.rs`
  - `src-tauri/src/utils/dirs.rs`
  - `src-tauri/src/utils/resolve.rs`
- Frontend behavior and layout:
  - `src/assets/styles/index.scss`
  - `src/components/base/base-notice.tsx`
  - `src/components/connection/connection-table.tsx`
  - `src/components/layout/use-custom-theme.ts`
  - `src/components/profile/groups-editor-viewer.tsx`
  - `src/components/proxy/proxy-groups.tsx`
  - `src/components/proxy/proxy-head.tsx`
  - `src/components/proxy/proxy-item-mini.tsx`
  - `src/components/proxy/proxy-item.tsx`
  - `src/components/proxy/proxy-render.tsx`
  - `src/components/proxy/use-head-state.ts`
  - `src/components/proxy/use-window-width.ts`
  - `src/components/setting/mods/clash-port-viewer.tsx`
  - `src/components/setting/mods/misc-viewer.tsx`
  - `src/components/setting/mods/web-ui-viewer.tsx`
  - `src/components/setting/setting-clash.tsx`
  - `src/hooks/use-profiles.ts`
  - `src/pages/_layout.tsx`
  - `src/pages/connections.tsx`
  - `src/pages/logs.tsx`
  - `src/pages/profiles.tsx`
  - `src/services/cmds.ts`
  - `src/services/states.ts`
- Documentation:
  - `README.md`
  - `CONTRIBUTING.md`
  - `UPDATELOG.md`

Do not add `CLAUDE.md` to the rebuilt long-term branch unless the user
separately asks for repository-level AI coding guidance. It is not required for
the application or release process.

---

### Task 1: Create The Isolated Rebuild Branch

**Files:**

- Modify: none in the current worktree
- Create: external worktree `../clash-verge-rev-rebuild-1x-legacy`

- [ ] **Step 1: Confirm the current worktree has unrelated changes**

Run from `D:\Me\clash-verge-rev`:

```powershell
git status --short --branch
```

Expected: output includes local changes such as `.github/workflows/release.yml`,
`src-tauri/Cargo.toml`, or `release_notes.local.md`. Do not stage or modify
them.

- [ ] **Step 2: Fetch the source branches**

```powershell
git fetch origin release/1.x-legacy release/1.7.x-legacy
```

Expected: command succeeds.

- [ ] **Step 3: Create a local backup pointer for the old branch**

```powershell
git branch -f backup/release-1.x-legacy-before-rebuild origin/release/1.x-legacy
git rev-parse --short backup/release-1.x-legacy-before-rebuild
```

Expected: prints `84ce102e` unless the remote branch has moved after the spec
was written. If it differs, stop and inspect:

```powershell
git log --oneline 84ce102e..origin/release/1.x-legacy
```

- [ ] **Step 4: Create an isolated worktree from the clean baseline**

First confirm the branch and target directory are not already present:

```powershell
git branch --list work/rebuild-1x-legacy
Test-Path ..\clash-verge-rev-rebuild-1x-legacy
```

Expected: no branch output and `False` for the path check. If the branch or
path already exists, stop and inspect it before continuing:

```powershell
git worktree list
git log -1 --format="%h %d %s" work/rebuild-1x-legacy
```

Do not delete or overwrite an existing worktree without explicit approval.

```powershell
git worktree add ..\clash-verge-rev-rebuild-1x-legacy -b work/rebuild-1x-legacy release/1.7.x-legacy
```

Expected: new worktree created at `D:\Me\clash-verge-rev-rebuild-1x-legacy`.

- [ ] **Step 5: Confirm the isolated worktree baseline**

Run from `D:\Me\clash-verge-rev-rebuild-1x-legacy`:

```powershell
git status --short --branch
git log -1 --format="%h %d %s"
```

Expected:

```text
## work/rebuild-1x-legacy
7d2b1875 ... Release 1.7.7
```

No local changes should be present.

- [ ] **Step 6: Commit**

No commit is expected in this task. The output is the isolated branch itself.

---

### Task 2: Migrate Release And Packaging Infrastructure

**Files:**

- Create: `.github/workflows/release-1x-legacy.yml`
- Create: `scripts/build.mjs`
- Create: `scripts/prepare-legacy-release.mjs`
- Create: `scripts/portable-legacy.mjs`
- Create: `scripts/portable-fixed-webview2-legacy.mjs`
- Create: `scripts/updater-legacy.mjs`
- Create: `scripts/updater-fixed-webview2-legacy.mjs`
- Create: `scripts/print-updatelog.mjs`
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `scripts/check.mjs`
- Modify: `scripts/updatelog.mjs`

- [ ] **Step 1: Restore packaging files from the source branch**

Run from `D:\Me\clash-verge-rev-rebuild-1x-legacy`:

```powershell
git restore --source origin/release/1.x-legacy -- `
  .github/workflows/release-1x-legacy.yml `
  package.json `
  pnpm-lock.yaml `
  scripts/build.mjs `
  scripts/check.mjs `
  scripts/prepare-legacy-release.mjs `
  scripts/portable-legacy.mjs `
  scripts/portable-fixed-webview2-legacy.mjs `
  scripts/updater-legacy.mjs `
  scripts/updater-fixed-webview2-legacy.mjs `
  scripts/print-updatelog.mjs `
  scripts/updatelog.mjs
```

Expected: files are restored into the worktree.

- [ ] **Step 2: Inspect the release script naming**

```powershell
Select-String -Path scripts\prepare-legacy-release.mjs -Pattern "LEGACY_PRODUCT_NAME|LEGACY_IDENTIFIER|REPO_OWNER|REPO_NAME"
Select-String -Path scripts\portable-legacy.mjs -Pattern "APP_EXE_NAME|ZIP_PREFIX"
Select-String -Path .github\workflows\release-1x-legacy.yml -Pattern "legacy:prepare-release|legacy:portable|legacy:updater"
```

Expected: confirms the legacy release path is explicit and does not rely on
mainline `release.yml`.

- [ ] **Step 3: Keep build wrapper behavior**

Inspect `package.json`:

```powershell
node -e "const p=require('./package.json'); console.log(p.scripts.build); console.log(p.scripts['legacy:prepare-release']);"
```

Expected:

```text
node scripts/build.mjs
node scripts/prepare-legacy-release.mjs
```

The wrapper is retained because `build.rs` will require resources to exist
before `tauri build`.

- [ ] **Step 4: Validate JavaScript syntax**

```powershell
node --check scripts/build.mjs
node --check scripts/check.mjs
node --check scripts/prepare-legacy-release.mjs
node --check scripts/portable-legacy.mjs
node --check scripts/portable-fixed-webview2-legacy.mjs
node --check scripts/updater-legacy.mjs
node --check scripts/updater-fixed-webview2-legacy.mjs
node --check scripts/print-updatelog.mjs
node --check scripts/updatelog.mjs
```

Expected: no syntax errors.

- [ ] **Step 5: Inspect diff scope**

```powershell
git diff --name-status
```

Expected: only files listed in this task are changed.

- [ ] **Step 6: Commit**

```powershell
git add .github/workflows/release-1x-legacy.yml package.json pnpm-lock.yaml scripts
git commit -m "build: add clean 1x legacy release pipeline"
```

Expected: one commit containing only packaging and release infrastructure.

---

### Task 3: Migrate Backend Identity, Runtime Artifacts, And Config Defaults

**Files:**

- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/build.rs`
- Modify: `src-tauri/src/config/clash.rs`
- Modify: `src-tauri/src/config/verge.rs`
- Modify: `src-tauri/src/core/clash_api.rs`
- Modify: `src-tauri/src/core/core.rs`
- Modify: `src-tauri/src/core/sysopt.rs`
- Modify: `src-tauri/src/core/tray.rs`
- Modify: `src-tauri/src/feat.rs`
- Modify: `src-tauri/src/utils/dirs.rs`

- [ ] **Step 1: Restore backend files except window command wiring**

```powershell
git restore --source origin/release/1.x-legacy -- `
  src-tauri/Cargo.toml `
  src-tauri/Cargo.lock `
  src-tauri/build.rs `
  src-tauri/src/config/clash.rs `
  src-tauri/src/config/verge.rs `
  src-tauri/src/core/clash_api.rs `
  src-tauri/src/core/core.rs `
  src-tauri/src/core/sysopt.rs `
  src-tauri/src/core/tray.rs `
  src-tauri/src/feat.rs `
  src-tauri/src/utils/dirs.rs
```

Expected: backend support changes are restored, but `main.rs`,
`cmds.rs`, and `resolve.rs` are left for the window/tray task.

- [ ] **Step 2: Confirm legacy application identity isolation**

```powershell
Select-String -Path src-tauri\src\utils\dirs.rs -Pattern "APP_ID"
```

Expected: production and dev app IDs include `clash-verge-rev-legacy`, so this
branch does not share app data with the non-legacy line.

- [ ] **Step 3: Confirm auto-launch uses silent argument**

```powershell
Select-String -Path src-tauri\src\core\sysopt.rs -Pattern "set_args"
```

Expected: `set_args(&["--silent"])` is present.

- [ ] **Step 4: Confirm build-time runtime artifact checks**

```powershell
Select-String -Path src-tauri\build.rs -Pattern "ensure_runtime_artifacts|pnpm check"
```

Expected: `build.rs` checks for sidecars/resources and tells developers to run
`pnpm check <target>` or `pnpm build`.

- [ ] **Step 5: Confirm intended config default changes**

```powershell
Select-String -Path src-tauri\src\config\verge.rs -Pattern "verge_mixed_port|verge_socks_port|verge_port|auto_log_clean"
```

Expected: default ports are aligned to classic Clash conventions:

```text
verge_mixed_port: Some(7890)
verge_socks_port: Some(7891)
verge_port: Some(7892)
auto_log_clean: Some(1)
```

- [ ] **Step 6: Run Rust metadata check**

```powershell
Push-Location src-tauri
cargo metadata --no-deps
Pop-Location
```

Expected: cargo metadata succeeds.

- [ ] **Step 7: Inspect diff scope**

```powershell
git diff --name-status
```

Expected: changes are limited to this task plus the already committed release
pipeline history. If `main.rs`, `cmds.rs`, or `resolve.rs` appear now, stop and
verify they were not restored accidentally in this task.

- [ ] **Step 8: Commit**

```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/build.rs src-tauri/src/config src-tauri/src/core src-tauri/src/feat.rs src-tauri/src/utils/dirs.rs
git commit -m "backend: isolate 1x legacy runtime configuration"
```

Expected: one backend/config commit.

---

### Task 4: Migrate Window, Tray, Silent Start, And Warm-To-Tray Behavior

**Files:**

- Modify: `src-tauri/src/cmds.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/utils/resolve.rs`
- Modify: `src/pages/_layout.tsx`
- Modify: `src/services/cmds.ts`

- [ ] **Step 1: Restore the window and command files**

```powershell
git restore --source origin/release/1.x-legacy -- `
  src-tauri/src/cmds.rs `
  src-tauri/src/main.rs `
  src-tauri/src/utils/resolve.rs `
  src/pages/_layout.tsx `
  src/services/cmds.ts
```

Expected: final `.21` warm-to-tray behavior is restored.

- [ ] **Step 2: Confirm warm-to-tray command wiring**

```powershell
Select-String -Path src-tauri\src\cmds.rs -Pattern "is_warm_to_tray"
Select-String -Path src-tauri\src\main.rs -Pattern "is_warm_to_tray"
Select-String -Path src\services\cmds.ts -Pattern "is_warm_to_tray|isWarmToTray"
Select-String -Path src\pages\_layout.tsx -Pattern "isWarmToTray|setFocus"
```

Expected:

- Rust command `is_warm_to_tray` exists.
- `main.rs` registers it in `generate_handler`.
- TypeScript service exposes `isWarmToTray`.
- `_layout.tsx` skips focus during warm-to-tray startup.

- [ ] **Step 3: Confirm close-to-tray does not destroy the window**

```powershell
Select-String -Path src-tauri\src\main.rs -Pattern "CloseRequested|prevent_close|set_position"
```

Expected: close requests call `api.prevent_close()` and move the window
offscreen on Windows.

- [ ] **Step 4: Confirm Windows taskbar skip bypasses Tauri hide path**

```powershell
Select-String -Path src-tauri\src\utils\resolve.rs -Pattern "set_window_taskbar_skip|SetWindowLongPtrW|SW_SHOWNOACTIVATE|WARM_TO_TRAY"
```

Expected:

- `set_window_taskbar_skip` uses `SetWindowLongPtrW` and `SetWindowPos`.
- warm-to-tray uses `ShowWindow(..., SW_SHOWNOACTIVATE)`.
- `WARM_TO_TRAY` is set from `enable_silent_start && --silent`.

- [ ] **Step 5: Confirm offscreen position is not persisted**

```powershell
Select-String -Path src-tauri\src\utils\resolve.rs -Pattern "-10000|-32000"
```

Expected: `save_window_size_position` returns early for offscreen positions.

- [ ] **Step 6: Run frontend type check**

```powershell
pnpm exec tsc --noEmit
```

Expected: TypeScript succeeds. If this project expects `pnpm web:build` instead,
run:

```powershell
pnpm web:build
```

- [ ] **Step 7: Run Rust check**

```powershell
Push-Location src-tauri
cargo check
Pop-Location
```

Expected: Rust check succeeds.

- [ ] **Step 8: Commit**

```powershell
git add src-tauri/src/cmds.rs src-tauri/src/main.rs src-tauri/src/utils/resolve.rs src/pages/_layout.tsx src/services/cmds.ts
git commit -m "window: preserve WebView2 for 1x legacy tray startup"
```

Expected: one commit for window/tray/silent-start behavior.

---

### Task 5: Migrate Profile Switching And Proxy Experience Fixes

**Files:**

- Modify: `src/hooks/use-profiles.ts`
- Modify: `src/pages/profiles.tsx`
- Modify: `src/components/profile/groups-editor-viewer.tsx`
- Modify: `src/components/proxy/proxy-groups.tsx`
- Modify: `src/components/proxy/proxy-head.tsx`
- Modify: `src/components/proxy/proxy-item-mini.tsx`
- Modify: `src/components/proxy/proxy-item.tsx`
- Modify: `src/components/proxy/proxy-render.tsx`
- Modify: `src/components/proxy/use-head-state.ts`
- Modify: `src/components/proxy/use-window-width.ts`
- Modify: `src/components/setting/mods/clash-port-viewer.tsx`
- Modify: `src/components/setting/mods/misc-viewer.tsx`
- Modify: `src/components/setting/mods/web-ui-viewer.tsx`
- Modify: `src/components/setting/setting-clash.tsx`

- [ ] **Step 1: Restore profile, proxy, and setting files**

```powershell
git restore --source origin/release/1.x-legacy -- `
  src/hooks/use-profiles.ts `
  src/pages/profiles.tsx `
  src/components/profile/groups-editor-viewer.tsx `
  src/components/proxy/proxy-groups.tsx `
  src/components/proxy/proxy-head.tsx `
  src/components/proxy/proxy-item-mini.tsx `
  src/components/proxy/proxy-item.tsx `
  src/components/proxy/proxy-render.tsx `
  src/components/proxy/use-head-state.ts `
  src/components/proxy/use-window-width.ts `
  src/components/setting/mods/clash-port-viewer.tsx `
  src/components/setting/mods/misc-viewer.tsx `
  src/components/setting/mods/web-ui-viewer.tsx `
  src/components/setting/setting-clash.tsx
```

Expected: profile switching and proxy layout fixes are restored.

- [ ] **Step 2: Confirm import does not auto-switch existing users**

```powershell
Select-String -Path src\pages\profiles.tsx -Pattern "!newProfiles.current|newRemote|setProfilesCurrentOptimistic"
```

Expected: automatic selection happens only when `!newProfiles.current`.

- [ ] **Step 3: Confirm profile switch is locked and optimistic**

```powershell
Select-String -Path src\pages\profiles.tsx -Pattern "useLockFn|previousCurrent|setActivatings|activateSelected"
```

Expected: profile select path uses `useLockFn`, optimistic current update,
rollback on failure, and locked selector restoration.

- [ ] **Step 4: Confirm selector restoration retries briefly**

```powershell
Select-String -Path src\hooks\use-profiles.ts -Pattern "setTimeout|groups?.length|Promise.all"
```

Expected: selector restoration retries empty proxy groups and waits for pending
selector updates before persisting selected state.

- [ ] **Step 5: Confirm proxy layout stability fixes**

```powershell
Select-String -Path src\components\proxy\proxy-item.tsx -Pattern "boxShadow|default_latency_timeout"
Select-String -Path src\components\proxy\proxy-item-mini.tsx -Pattern "boxShadow|default_latency_timeout"
Select-String -Path src\components\proxy\use-head-state.ts -Pattern "useLayoutEffect"
Select-String -Path src\components\proxy\use-window-width.ts -Pattern "ResizeObserver"
Select-String -Path src\components\proxy\proxy-render.tsx -Pattern "height=\"32px\""
```

Expected: selected-row shift is replaced by inset `boxShadow`, initial head
state hydrates before paint, body width uses `ResizeObserver`, and proxy icons
have stable dimensions.

- [ ] **Step 6: Run frontend type check**

```powershell
pnpm exec tsc --noEmit
```

Expected: TypeScript succeeds.

- [ ] **Step 7: Commit**

```powershell
git add src/hooks/use-profiles.ts src/pages/profiles.tsx src/components/profile/groups-editor-viewer.tsx src/components/proxy src/components/setting
git commit -m "frontend: stabilize legacy profile and proxy flows"
```

Expected: one commit for profile/proxy/settings experience fixes.

---

### Task 6: Migrate Shared UI Stability Fixes

**Files:**

- Modify: `src/assets/styles/index.scss`
- Modify: `src/components/base/base-notice.tsx`
- Modify: `src/components/connection/connection-table.tsx`
- Modify: `src/components/layout/use-custom-theme.ts`
- Modify: `src/pages/connections.tsx`
- Modify: `src/pages/logs.tsx`
- Modify: `src/services/states.ts`

- [ ] **Step 1: Restore shared UI files**

```powershell
git restore --source origin/release/1.x-legacy -- `
  src/assets/styles/index.scss `
  src/components/base/base-notice.tsx `
  src/components/connection/connection-table.tsx `
  src/components/layout/use-custom-theme.ts `
  src/pages/connections.tsx `
  src/pages/logs.tsx `
  src/services/states.ts
```

Expected: shared UI stability changes are restored.

- [ ] **Step 2: Confirm info notice layout matches other variants**

```powershell
Select-String -Path src\components\base\base-notice.tsx -Pattern "InfoRounded|type === \"info\"|width: 328"
```

Expected: info notices use the same icon/text box as success and error.

- [ ] **Step 3: Confirm first-paint background is external CSS**

```powershell
Select-String -Path src\assets\styles\index.scss -Pattern "prefers-color-scheme|body"
Select-String -Path src\components\layout\use-custom-theme.ts -Pattern "default"
```

Expected: first-paint background lives in SCSS and theme background includes a
default color.

- [ ] **Step 4: Confirm connection sorting does not mutate source data**

```powershell
Select-String -Path src\pages\connections.tsx -Pattern ".slice"
Select-String -Path src\components\connection\connection-table.tsx -Pattern "background|DataGrid"
```

Expected: filtered connections are copied before sorting, and table background
fixes are present.

- [ ] **Step 5: Confirm log level persists**

```powershell
Select-String -Path src\pages\logs.tsx -Pattern "useLogLevel"
Select-String -Path src\services\states.ts -Pattern "useLogLevel"
```

Expected: log page uses shared persisted state rather than local `useState`.

- [ ] **Step 6: Run frontend type check**

```powershell
pnpm exec tsc --noEmit
```

Expected: TypeScript succeeds.

- [ ] **Step 7: Commit**

```powershell
git add src/assets/styles/index.scss src/components/base/base-notice.tsx src/components/connection/connection-table.tsx src/components/layout/use-custom-theme.ts src/pages/connections.tsx src/pages/logs.tsx src/services/states.ts
git commit -m "frontend: preserve legacy UI stability fixes"
```

Expected: one commit for shared UI stability.

---

### Task 7: Write Clean Long-Term Branch Documentation

**Files:**

- Modify: `README.md`
- Modify: `CONTRIBUTING.md`
- Modify: `UPDATELOG.md`

- [ ] **Step 1: Restore small build guidance changes**

```powershell
git restore --source origin/release/1.x-legacy -- README.md CONTRIBUTING.md
```

Expected: build docs mention that `pnpm build` prepares resources and sidecars.

- [ ] **Step 2: Rewrite the top legacy changelog section**

Edit `UPDATELOG.md` manually. Replace the current top legacy section with a
single clean long-term branch entry:

```markdown
## v1.7.7-legacy-rebuild

### Notice

- Rebuilds the 1.x legacy maintenance branch from the `v1.7.7` baseline with a cleaner, functional commit history.
- Preserves the intended legacy release flow, app identity isolation, Windows tray behavior, and user-facing stability fixes from the previous `release/1.x-legacy` line.
- The old `.legacy.1` through `.legacy.21` release notes are treated as implementation history, not as the structure of this rebuilt branch.

### Build / Release

- Adds the dedicated 1.x legacy release workflow and legacy portable/updater scripts.
- `pnpm build` prepares required sidecar and resource files before Tauri packaging.
- The release preparation script rewrites package metadata for legacy artifacts.

### Runtime

- Uses a legacy app identifier so the branch does not share configuration with the non-legacy app line.
- Registers auto-launch with `--silent`, so silent startup only applies to boot auto-launch.
- Keeps Windows close-to-tray and warm-to-tray paths alive offscreen to avoid WebView2 cold starts when reopening from the tray.

### Frontend

- Stabilizes profile switching, selector restoration, proxy list layout, connection sorting, log level persistence, notices, and first-paint theme background.
```

Keep the existing `## v1.7.7` and older sections below this new entry.

- [ ] **Step 3: Ensure old process changelog was not copied verbatim**

```powershell
Select-String -Path UPDATELOG.md -Pattern "legacy.21|legacy.20|legacy.19|第二十|第十九"
```

Expected: no matches in the new top section. If matches exist only in preserved
historic sections below and are intentionally retained, document that in the
commit message body.

- [ ] **Step 4: Inspect docs diff**

```powershell
git diff -- README.md CONTRIBUTING.md UPDATELOG.md
```

Expected: README/CONTRIBUTING contain concise build guidance; UPDATELOG has a
clean rebuilt branch entry instead of the full `.legacy.1` through `.legacy.21`
process history.

- [ ] **Step 5: Commit**

```powershell
git add README.md CONTRIBUTING.md UPDATELOG.md
git commit -m "docs: summarize rebuilt 1x legacy branch"
```

Expected: one documentation commit.

---

### Task 8: Run Final Automated Verification

**Files:**

- Modify: none unless checks reveal issues

- [ ] **Step 1: Confirm branch and commit shape**

```powershell
git branch --show-current
git log --oneline release/1.7.x-legacy..HEAD
```

Expected: branch is `work/rebuild-1x-legacy`, and commits are focused by
functional area, not old `.legacy.N` release sequence.

- [ ] **Step 2: Confirm no `CLAUDE.md` was added**

```powershell
Test-Path CLAUDE.md
```

Expected:

```text
False
```

- [ ] **Step 3: Compare final file set against intended source diff**

```powershell
git diff --name-status release/1.7.x-legacy..HEAD
git diff --name-status release/1.7.x-legacy..origin/release/1.x-legacy
```

Expected: rebuilt branch includes the useful application and release files, but
does not include `CLAUDE.md` unless explicitly requested.

- [ ] **Step 4: Run project check**

For the host target:

```powershell
pnpm check --force
```

Expected: downloads or refreshes runtime resources and exits successfully. This
may perform network downloads.

For a Windows release target, if resources are needed:

```powershell
pnpm check x86_64-pc-windows-msvc --force
```

Expected: target-specific sidecars and resources resolve successfully.

- [ ] **Step 5: Run frontend build**

```powershell
pnpm web:build
```

Expected: TypeScript and Vite build succeed.

- [ ] **Step 6: Run Rust/Tauri check**

```powershell
Push-Location src-tauri
cargo check
Pop-Location
```

Expected: Rust check succeeds.

- [ ] **Step 7: Run package build only if full build cost is acceptable**

```powershell
pnpm build --target x86_64-pc-windows-msvc
```

Expected: `scripts/build.mjs` runs `pnpm check x86_64-pc-windows-msvc` and then
`pnpm tauri build --target x86_64-pc-windows-msvc`.

If the full Tauri build is too expensive, record it as not run and do not claim
package build verification.

- [ ] **Step 8: Commit fixes if verification required changes**

If verification required changes, commit them with the smallest relevant scope:

```powershell
git add <changed-files>
git commit -m "fix: resolve rebuilt legacy verification issues"
```

Expected: no unrelated files are included.

---

### Task 9: Manual Windows Behavior Verification

**Files:**

- Modify: none unless manual verification reveals issues

- [ ] **Step 1: Launch normally**

Run:

```powershell
pnpm dev
```

Expected: app opens normally when not passed `--silent`.

- [ ] **Step 2: Verify manual startup is not silent**

With `enable_silent_start` enabled in settings, close the app, then launch
without `--silent`.

Expected: main window appears. It should not stay tray-only.

- [ ] **Step 3: Verify boot-style silent startup**

Launch using the silent argument:

```powershell
pnpm tauri dev -- --silent
```

Expected: app starts without stealing focus. The main WebView2 process should
be warmed before the first tray-open action.

- [ ] **Step 4: Verify close-to-tray**

Click the window close button.

Expected:

- app remains running;
- main window disappears from the visible desktop;
- tray entry can reopen it;
- reopen is immediate and does not show a black border flash;
- previous SPA state is preserved where expected.

- [ ] **Step 5: Verify saved window position**

Move the window to a visible position, close to tray, reopen, then restart.

Expected: the app does not persist `-32000` offscreen coordinates and reopens at
the last visible position.

- [ ] **Step 6: Record manual verification result**

Create a local note outside the repo or in an untracked file:

```text
Manual Windows verification:
- normal launch:
- --silent launch:
- close-to-tray:
- tray reopen:
- saved position:
- WebView2 warm-up:
```

Expected: the final response reports which checks were run and which were not.

---

### Task 10: Prepare Branch Replacement For Explicit Approval

**Files:**

- Modify: none

- [ ] **Step 1: Confirm backup pointer still exists**

```powershell
git rev-parse --short backup/release-1.x-legacy-before-rebuild
```

Expected: prints the old branch commit, originally `84ce102e`.

- [ ] **Step 2: Show replacement summary**

```powershell
git log --oneline release/1.7.x-legacy..work/rebuild-1x-legacy
git diff --stat origin/release/1.x-legacy..work/rebuild-1x-legacy
```

Expected: focused commit list and a clear summary of how the rebuilt branch
differs from the old remote branch.

- [ ] **Step 3: Stop before updating the remote branch**

Do not run `git push --force-with-lease` yet.

Prepare this command for explicit user approval only:

```powershell
git push --force-with-lease origin work/rebuild-1x-legacy:release/1.x-legacy
```

Expected: user explicitly approves before remote branch replacement.
