# Legacy Cleanup Risk Review

Date: 2026-05-24
Branch reviewed: `release/1.x-legacy`
Current HEAD: `84ce102e` (`Release 1.7.7-legacy.21`)
Clean baseline: `v1.7.7` (`41f8cc29`)
Safety backup: `backup/messy-legacy-2026-05-24`
Recovery worktree: `C:\Users\xqd\.config\superpowers\worktrees\clash-verge-rev-1.7.7\legacy-clean-2026-05-24`
Recovery branch: `recovery/legacy-clean-2026-05-24`

## Skill Check Log

Before touching the repository for this cleanup, the following skills were checked and applied where relevant:

- `using-superpowers`: required by the user's "check skills before every action" instruction.
- `code-reviewer`: used for reviewing the accumulated local branch changes.
- `diagnose` and `systematic-debugging`: used to avoid guessing fixes without a repro loop.
- `frontend-code-review`: used because the diff includes React/TypeScript UI changes.
- `using-git-worktrees`: checked for isolation guidance and used to create an isolated recovery worktree from `v1.7.7`.
- `verification-before-completion`: used for final status checks before claiming what was done.

## Progress Log

- Created safety branch `backup/messy-legacy-2026-05-24` at current messy `HEAD` (`84ce102e`).
- Created recovery branch `recovery/legacy-clean-2026-05-24` from `v1.7.7`.
- Created isolated recovery worktree at `C:\Users\xqd\.config\superpowers\worktrees\clash-verge-rev-1.7.7\legacy-clean-2026-05-24`.
- Replayed and committed the first low-risk group as `c695f5d9` (`chore(legacy): restore safe foundation changes`):
  - `CLAUDE.md`;
  - legacy APP ID rename;
  - tray wording `Restart Core`;
  - log level persistence;
  - Notice info layout alignment.
- Verification for `c695f5d9`:
  - `pnpm install --frozen-lockfile --prefer-offline`: passed.
  - `pnpm check x86_64-pc-windows-msvc`: passed and prepared required sidecar/resources.
  - `$env:NODE_OPTIONS='--max_old_space_size=8192'; pnpm web:build`: passed.
  - `cargo test` in `src-tauri`: passed, 6 tests.

## Scope

This review covers the `xqd` authored changes from `v1.7.7` to current `HEAD`.

Summary of the diff:

- 66 `xqd` commits after `v1.7.7`.
- 53 files changed.
- 2146 insertions, 202 deletions.
- Commit mix: 24 `fix`, 16 release commits, 6 reverts, 4 CI commits, 4 tuning commits, 4 docs commits.

The large number of fix/revert commits in a short period is the main signal that this branch should not be cleaned by continuing to patch on top. It should be reconstructed from the clean baseline with grouped, verified changes.

## Recommended Strategy

1. Keep `release/1.x-legacy` untouched as the current messy branch.
2. Keep `backup/messy-legacy-2026-05-24` as the recovery point for the exact current state.
3. Create `recovery/legacy-clean-2026-05-24` from `v1.7.7`.
4. Reapply only low-risk and proven changes in small groups.
5. Rebuild high-risk changes from scratch only after a reproducible check exists.
6. Squash the final clean branch into a small number of meaningful commits.

## Risk Classification

### Revert First

These changes should not be carried into the clean branch until they have a specific repro and verification path.

#### Window and Warm-to-Tray Lifecycle

Related commits:

- `e7879fb0` `fix(window): intercept CloseRequested to hide instead of destroy`
- `36bc69c0` `fix(window): re-attempt close-to-tray with minimize+skip_taskbar+hide`
- `8dab68ee` `revert(window): restore v1.7.7 default close to fix tray-open black line`
- `ac8a7497` `fix(window): close-to-tray via offscreen positioning`
- `5a922694` `fix(launch): warm-to-tray mode preheats WebView2 on silent autostart`
- `b5575df4` `fix(launch): warm-to-tray force WebView2 init via SW_SHOWNOACTIVATE`
- `e2c46caa` `fix(window): bypass tauri set_skip_taskbar to keep DWM compositor alive`

Evidence:

- `src-tauri/src/main.rs:139` now prevents close and moves the window to `(-32000, -32000)`.
- `src-tauri/src/utils/resolve.rs:89` introduces `warm_to_tray`.
- `src-tauri/src/utils/resolve.rs:102` skips the taskbar by directly mutating Win32 extended styles.
- `src-tauri/src/utils/resolve.rs:110` calls `ShowWindow(..., SW_SHOWNOACTIVATE)`.
- `src/pages/_layout.tsx:94` calls `appWindow.show()` in warm-to-tray mode while trying to avoid focus.

Risk:

- This touches Tauri lifecycle, Win32 window styles, taskbar behavior, WebView2 startup, focus behavior, and tray restore all at once.
- The sequence already shows failed attempts and a revert, so the current result is not a stable incremental fix.
- Offscreen-visible windows can introduce invisible focus, stale saved positions, taskbar state mismatches, or black/blank WebView restore behavior.

Recommendation:

- Revert all window/warm-to-tray behavior to the original `v1.7.7` behavior in the clean branch.
- Reintroduce only one minimal fix at a time after creating a manual or automated checklist:
  - normal launch shows and focuses the window;
  - close button behavior;
  - tray click restore behavior;
  - boot auto-launch silent behavior;
  - no black line or blank WebView after restore.

#### Global SWR Focus Revalidation Disable

Related commit:

- `a34c3dab` `fix(swr): disable revalidateOnFocus to stop IPC storm on window focus`

Evidence:

- `src/pages/_layout.tsx:115` sets `revalidateOnFocus: false` globally.

Risk:

- This can hide stale state bugs by preventing refresh after focus.
- It changes behavior for every SWR consumer, not only the IPC-heavy path.

Recommendation:

- Do not carry the global setting into the clean branch.
- If IPC storms are real, fix the specific SWR keys or event loop that storms.

#### Profile Switch Flow Rewrite

Related commits:

- `8539aa3f` `perf(profiles): streamline profile switching flow`
- `704d1702` `fix(profiles): harden switch flow against races and silent failures`
- `c549e855` `fix(cmds): return latest profiles snapshot to avoid stale UI during switch`
- `6a6c5ccf` `revert(profiles): drop "Selectors Restore Failed" notice on switch`

Evidence:

- `src-tauri/src/cmds.rs:24` changes `get_profiles` from persisted `data()` to draft-aware `latest()`.
- `src/pages/profiles.tsx:119` adds optimistic current-profile mutation.
- `src/pages/profiles.tsx:187` switches UI state before `patchProfiles` succeeds.
- `src/hooks/use-profiles.ts:28` adds polling and selector restoration retries.
- `src/hooks/use-profiles.ts:64` silently swallows selector restore failure through caller catches.

Risk:

- Backend draft state is now exposed to normal frontend reads.
- Optimistic UI can show a current profile that failed to apply, then rely on rollback timing.
- Selector restore failures are treated as non-fatal, which can hide profile switch partial failure.
- The flow combines state mutation, mihomo hot reload, selector restore, connection close, and SWR mutation.

Recommendation:

- Revert this group in the clean branch.
- Rebuild profile switching around an explicit state machine:
  - idle;
  - switching current profile;
  - applying mihomo config;
  - restoring selectors;
  - succeeded or failed with rollback.
- Keep `get_profiles` returning committed persisted state unless a caller explicitly requests draft state.

#### Async Config Validation During Hot Reload

Related commit:

- `afd99e61` `perf(core): bound put_configs latency with single PUT and 30s timeout`

Evidence:

- `src-tauri/src/core/core.rs:305` moves `check_config()` into `spawn_blocking`.
- `src-tauri/src/core/core.rs:315` reduces hot reload to one `put_configs` call.
- `src-tauri/src/core/clash_api.rs:9` adds 30s PUT timeout and 3s PATCH timeout.

Risk:

- Validation no longer blocks applying the runtime config.
- A validation warning can arrive after the user sees a successful switch/update path.
- The retry removal may be correct, but it changed both validation ordering and retry behavior in one commit.

Recommendation:

- Treat as `redo`, not blind keep.
- Split into two independent changes later:
  - add request timeouts;
  - change retry/validation behavior.

### Inspect Before Keeping

These changes may be useful, but should be reintroduced only after they are isolated.

#### Legacy Release Workflow and Build Scripts

Related commits:

- `9607280e` `build(legacy): add dedicated 1.x release workflow`
- `2aa2cb9e` `fix(build): harden local packaging pipeline`
- `7ddcfc8c`, `dd8f0929`, `adf5c870`, `5d593541`, `ac78b2a6`

Evidence:

- `.github/workflows/release-1x-legacy.yml:20` uses `permissions: write-all`.
- `.github/workflows/release-1x-legacy.yml:18` defaults `overwrite_existing` to `true`.
- `.github/workflows/release-1x-legacy.yml:73` deletes existing release and tag.
- `scripts/check.mjs:17` pins legacy service resources to `v1.7.7` by default.
- `scripts/check.mjs:387` extracts service binaries from legacy portable packages.

Risk:

- The workflow can delete tags/releases by default.
- Several CI commits and one revert show the release pipeline was unstable.
- The legacy resource download path is useful but should be tested independently.

Recommendation:

- Keep the concept, but rewrite the workflow conservatively:
  - default `overwrite_existing: false`;
  - permissions scoped to `contents: write`;
  - no release/tag deletion unless explicitly requested;
  - release prep script tested locally before workflow use.

#### Legacy Branding and App ID

Related commit:

- `1eba3274` `chore: rename legacy APP_ID and add CLAUDE.md guidelines`

Risk:

- App identity changes can affect auto-launch registration, config directories, updater identity, and installed app coexistence.

Recommendation:

- Keep only if the goal is explicit coexistence with upstream/mainline app.
- Verify config path, auto-launch entry, and updater endpoint before release.

#### Default Ports and Latency Defaults

Related commits:

- `b0375e30` lower latency timeout.
- `cf8d5413` switch default latency URL.
- `d14367a4` revert proxy defaults.
- `919453f2`, `ab6be8b1`, `b0330373` tune and revert misc defaults.
- `7823fbba` align default ports with Clash classic convention.

Evidence:

- `src-tauri/src/config/clash.rs:43` changes defaults to `7890/7891/7892/7893/7894` and controller `9090`.
- `src-tauri/src/config/verge.rs:235` changes app defaults to the same convention.
- `src/components/setting/mods/misc-viewer.tsx:211` uses `http://www.gstatic.com/generate_204`.
- `src/components/proxy/proxy-item.tsx:55` falls back to 2000ms when config value is `0`.

Risk:

- Port changes are user-visible and can collide with existing local Clash installations.
- The sequence contains revert commits, indicating defaults were not settled.

Recommendation:

- Keep only if classic Clash compatibility is a product requirement.
- Otherwise revert ports to `v1.7.7` defaults and keep the latency placeholder cleanup separately.

### Likely Keep

These changes look low-risk or narrowly scoped, but still need normal build checks in the clean branch.

- Release notes in `UPDATELOG.md`.
- `CLAUDE.md` project guidelines.
- `i18n(tray): rename "Restart Clash" to "Restart Core"`.
- `fix(logs): persist log level filter across page navigation`.
- `fix(notice): unify info variant layout with success/error`.
- Connection/proxy visual jitter fixes only if visual QA confirms they do not regress layout.
- `fix: pin legacy service binaries`, but preferably as part of a smaller build-script commit.

## Clean Branch Replay Plan

### Phase 1: Safe Foundation

Start from `v1.7.7` on `recovery/legacy-clean-2026-05-24`.

Apply:

- `CLAUDE.md`.
- Legacy release notes.
- Conservative legacy build script pieces that do not delete releases by default.
- Service binary pinning, with local check of `pnpm check x86_64-pc-windows-msvc`.

Do not apply:

- Window lifecycle changes.
- Warm-to-tray changes.
- Global `revalidateOnFocus: false`.
- Profile switch optimistic rewrite.

### Phase 2: Product Defaults

Decide one product policy before applying:

- If this branch should behave like Clash classic, apply port defaults `7890/7891/7892/9090`.
- If this branch should preserve v1.7.7 compatibility, keep original `7897/7898/7899/9097`.

Apply latency URL/timeout changes only after confirming the intended defaults.

### Phase 3: Profile Switching

Rebuild as a small explicit flow with verification. Required checks:

- switching to a valid profile updates current profile exactly once;
- failed switch rolls UI back;
- selector restoration failure is visible or logged;
- profile import does not unexpectedly switch if a current profile already exists;
- no stale `getProfiles` response after switch.

### Phase 4: Window and Boot Behavior

Treat this as the riskiest subsystem. Required checks:

- normal launch shows the window and focuses it;
- close button behavior matches product expectation;
- tray click restores immediately;
- boot auto-launch with `--silent` does not steal focus;
- WebView does not restore as black/blank;
- saved position is not overwritten with offscreen coordinates.

Do not ship this phase without a manual QA checklist at minimum.

## Immediate Next Actions

1. Keep current branch as-is.
2. Use `backup/messy-legacy-2026-05-24` if the current branch needs to be restored exactly.
3. Use `recovery/legacy-clean-2026-05-24` as the clean rebuild branch from `v1.7.7`.
4. First rebuild commit should contain only docs/release foundation.
5. Second rebuild commit should contain only conservative build pipeline changes.
6. High-risk runtime behavior should be reimplemented only after a reproducible check exists.

## Verification Gates

For every replay group:

- `pnpm web:build`
- `cargo test` in `src-tauri`
- `pnpm check x86_64-pc-windows-msvc` when build-resource changes are touched
- manual Windows checklist for tray/window changes

Do not claim a group is stable until the relevant gate has fresh output.
