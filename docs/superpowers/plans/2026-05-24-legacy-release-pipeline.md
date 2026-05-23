# Legacy Release Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore a conservative, locally verifiable 1.x legacy release pipeline on the clean recovery branch.

**Architecture:** Keep release-specific behavior in dedicated legacy scripts and one dedicated GitHub Actions workflow. Local build hardening stays in `scripts/build.mjs`, `scripts/check.mjs`, and `src-tauri/build.rs` so failures are caught before packaging.

**Tech Stack:** Node ESM scripts, pnpm, Tauri 1.x, Rust build script, GitHub Actions.

---

### Task 1: Add Build Wrapper And Runtime Artifact Guard

**Files:**

- Create: `scripts/build.mjs`
- Modify: `package.json`
- Modify: `README.md`
- Modify: `CONTRIBUTING.md`
- Modify: `src-tauri/build.rs`

- [ ] **Step 1: Add `scripts/build.mjs`**

Create a Node wrapper that:

- reads raw CLI args;
- finds `--target` or `-t`;
- runs `pnpm check [target]`;
- runs `pnpm tauri build ...args`;
- adds `--max_old_space_size=4096` if missing.

- [ ] **Step 2: Wire `package.json`**

Change:

```json
"build": "tauri build"
```

to:

```json
"build": "node scripts/build.mjs"
```

- [ ] **Step 3: Add documentation notes**

Add one sentence to `README.md` and `CONTRIBUTING.md` explaining that `pnpm build` prepares sidecars/resources and that direct `pnpm tauri build` requires `pnpm check [target]` first.

- [ ] **Step 4: Add `src-tauri/build.rs` checks**

Before `tauri_build::build()`, verify required files:

- `resources/Country.mmdb`
- `resources/geoip.dat`
- `resources/geosite.dat`
- Windows resources: `clash-verge-service.exe`, `install-service.exe`, `uninstall-service.exe`, `enableLoopback.exe`
- sidecars: `verge-mihomo-{target}{ext}`, `verge-mihomo-alpha-{target}{ext}`

- [ ] **Step 5: Verify**

Run:

```powershell
pnpm check x86_64-pc-windows-msvc
$env:NODE_OPTIONS='--max_old_space_size=8192'; pnpm web:build
cargo test
```

Expected:

- `pnpm check` exits 0.
- `pnpm web:build` exits 0.
- `cargo test` exits 0 with 6 tests.

### Task 2: Harden `scripts/check.mjs`

**Files:**

- Modify: `scripts/check.mjs`

- [ ] **Step 1: Add pinned version env vars**

Add:

- `META_VERSION`
- `META_ALPHA_VERSION`
- `META_RULES_TAG`
- `UWP_TOOL_TAG`

- [ ] **Step 2: Add download validation**

Add helpers:

- `ensureOk(response, url)`
- `ensureVersionString(version, label)`
- `looksLikeHtml(buffer)`

Use them in version fetches and downloads.

- [ ] **Step 3: Add legacy Windows service extraction**

Add:

- `LEGACY_SERVICE_TAG`
- `LEGACY_SERVICE_REPO`
- `resolveLegacyServiceTag()`
- `ensureLegacyWindowsServiceResources()`

Windows builds should extract `clash-verge-service.exe`, `install-service.exe`, and `uninstall-service.exe` from the compatible legacy portable zip.

- [ ] **Step 4: Verify**

Run:

```powershell
$env:LEGACY_SERVICE_TAG='v1.7.7'; pnpm check x86_64-pc-windows-msvc --force
```

Expected:

- exits 0;
- service binaries exist in `src-tauri/resources`.

### Task 3: Add Legacy Release Scripts

**Files:**

- Create: `scripts/prepare-legacy-release.mjs`
- Create: `scripts/print-updatelog.mjs`
- Create: `scripts/portable-legacy.mjs`
- Create: `scripts/portable-fixed-webview2-legacy.mjs`
- Create: `scripts/updater-legacy.mjs`
- Create: `scripts/updater-fixed-webview2-legacy.mjs`
- Modify: `package.json`

- [ ] **Step 1: Add package scripts**

Add:

```json
"legacy:prepare-release": "node scripts/prepare-legacy-release.mjs",
"legacy:portable": "node scripts/portable-legacy.mjs",
"legacy:portable-fixed-webview2": "node scripts/portable-fixed-webview2-legacy.mjs",
"legacy:updater": "node scripts/updater-legacy.mjs",
"legacy:updater-fixed-webview2": "node scripts/updater-fixed-webview2-legacy.mjs"
```

- [ ] **Step 2: Add prepare script**

`scripts/prepare-legacy-release.mjs` must:

- require a tag;
- reject tags without `-legacy.`;
- update `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, and `src-tauri/webview2.*.json`;
- set product name to `Clash Verge Rev Legacy`;
- set package name to `clash-verge-legacy`;
- set identifier to `io.github.xqd922.clash-verge-rev-legacy`;
- set updater endpoints under `updater-legacy`.

- [ ] **Step 3: Add helper scripts**

Add legacy portable/updater scripts copied from the existing mainline shape but with legacy names and updater assets.

- [ ] **Step 4: Verify prepare script in a reversible way**

Run:

```powershell
node scripts/prepare-legacy-release.mjs v1.7.7-legacy.99
git diff -- package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json src-tauri/webview2.x64.json src-tauri/webview2.x86.json src-tauri/webview2.arm64.json
git checkout -- package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json src-tauri/webview2.x64.json src-tauri/webview2.x86.json src-tauri/webview2.arm64.json
```

Expected:

- script exits 0;
- diff shows only expected metadata changes;
- checkout restores generated changes before committing source scripts.

### Task 4: Add Conservative GitHub Actions Workflow

**Files:**

- Create: `.github/workflows/release-1x-legacy.yml`

- [ ] **Step 1: Add workflow dispatch inputs**

Inputs:

- `tag`, default `v1.7.7-legacy.1`;
- `release_name`, default `Release 1.7.7 Legacy`;
- `overwrite_existing`, default `false`.

- [ ] **Step 2: Scope permissions**

Use:

```yaml
permissions:
  contents: write
```

- [ ] **Step 3: Keep deletion gated**

Only delete release/tag when:

```yaml
if: ${{ github.event.inputs.overwrite_existing == 'true' }}
```

- [ ] **Step 4: Pin legacy service resources**

Every Windows build check step uses:

```yaml
env:
  LEGACY_SERVICE_TAG: v1.7.7
```

- [ ] **Step 5: Verify workflow structure**

Run:

```powershell
Select-String -Path .github/workflows/release-1x-legacy.yml -Pattern "contents: write","default: false","deleteRelease","deleteRef","LEGACY_SERVICE_TAG"
```

Expected:

- finds scoped permission;
- finds default false;
- deletion calls exist only in the gated cleanup step;
- legacy service tag is present in build check steps.

### Task 5: Final Verification And Commit

**Files:**

- All files from Tasks 1-4
- Modify: `docs/legacy-cleanup-risk-review.md`

- [ ] **Step 1: Update cleanup report**

Append a progress entry for the conservative release pipeline restore and list verification commands.

- [ ] **Step 2: Run full verification**

Run:

```powershell
pnpm install --frozen-lockfile --prefer-offline
$env:LEGACY_SERVICE_TAG='v1.7.7'; pnpm check x86_64-pc-windows-msvc --force
$env:NODE_OPTIONS='--max_old_space_size=8192'; pnpm web:build
cargo test
```

- [ ] **Step 3: Commit**

Commit:

```bash
git add .
git commit -m "build(legacy): restore conservative release pipeline"
```
