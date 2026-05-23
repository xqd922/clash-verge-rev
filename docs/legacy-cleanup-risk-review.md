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
- `brainstorming`, `writing-plans`, and `executing-plans`: used for the approved conservative release-pipeline design and task-by-task execution.
- `fix`: checked before formatting and CI-style verification.
- `update-docs`: checked before updating this cleanup report.

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
- Rebuilt the conservative legacy release pipeline on `recovery/legacy-clean-2026-05-24`:
  - `pnpm build` now runs `scripts/build.mjs`, which prepares sidecars/resources before `tauri build`.
  - `src-tauri/build.rs` now fails early when required runtime artifacts are missing.
  - `scripts/check.mjs` now validates HTTP downloads, supports pinned core/rules/tool versions, and extracts Windows legacy service binaries from `v1.7.7` portable artifacts by default.
  - Added legacy release helper scripts for metadata preparation, portable bundles, updater metadata, and release-note printing.
  - Added `.github/workflows/release-1x-legacy.yml` with manual dispatch only, `permissions: contents: write`, `overwrite_existing` defaulting to `false`, and release/tag deletion only behind the explicit overwrite input.
- Verification for the conservative release pipeline:
  - `node --check` for changed Node scripts: passed.
  - `pnpm exec prettier --check ...`: passed.
  - `node scripts/prepare-legacy-release.mjs v1.7.7-legacy.99` in a disposable copy: passed and rewrote only expected legacy metadata.
  - `Select-String` workflow inspection: found `contents: write`, `default: false`, gated `deleteRelease`/`deleteRef`, and pinned `LEGACY_SERVICE_TAG`; no `write-all` match.
  - `pnpm install --frozen-lockfile --prefer-offline`: passed, with an existing Node `url.parse()` deprecation warning.
  - `$env:LEGACY_SERVICE_TAG='v1.7.7'; pnpm check x86_64-pc-windows-msvc --force`: passed and extracted Windows service binaries from `v1.7.7` portable artifacts.
  - `$env:NODE_OPTIONS='--max_old_space_size=8192'; pnpm web:build`: passed, with existing Vite/Browserslist/chunk-size warnings.
  - `cargo test` in `src-tauri`: passed, 6 tests; existing Rust warnings remain.
- Completed default-policy audit for the remaining messy runtime changes:
  - Recovery branch has no diff from `v1.7.7` for `src-tauri/src/config/clash.rs`, `src-tauri/src/config/verge.rs`, `src/components/setting/mods/misc-viewer.tsx`, or `src/components/proxy/proxy-item.tsx`.
  - Current recovery defaults remain `7895/7896/7897/7898/7899` with controller `127.0.0.1:9097`.
  - Current recovery latency timeout fallback remains `10000`, and the latency-test placeholder remains `http://1.1.1.1`.
  - Messy branch changes to classic Clash ports `7890/7891/7892/9090`, `gstatic` latency URL, and `2000ms` fallback are not replayed because they are product-policy changes, not proven bug fixes.
- Completed high-risk runtime exclusion audit:
  - Recovery branch has no diff from `v1.7.7` for the remaining high-risk runtime files: `src-tauri/src/main.rs`, `src-tauri/src/utils/resolve.rs`, `src/pages/_layout.tsx`, `src/pages/profiles.tsx`, `src/hooks/use-profiles.ts`, `src-tauri/src/cmds.rs`, `src-tauri/src/core/core.rs`, and `src-tauri/src/core/clash_api.rs`.
  - Window lifecycle and warm-to-tray changes remain excluded because they mix close handling, taskbar visibility, offscreen positioning, WebView2 warmup, and Win32 style mutation.
  - Global `revalidateOnFocus: false` remains excluded because it suppresses refresh behavior for every SWR consumer instead of fixing a specific IPC storm path.
  - Profile switching rewrites remain excluded because they expose draft backend state, use optimistic UI mutation during a multi-step core reload, and silently swallow selector restoration failures.
  - Async config validation and single-`PUT /configs` behavior remain excluded as a redo candidate because request timeout changes, validation ordering, and retry removal should be split and tested separately.
- Replayed the narrow proxy selected-row jitter fix instead of cherry-picking the full proxy jitter sequence:
  - Added `src/components/proxy/proxy-selected-style.ts` so `ProxyItem` and `ProxyItemMini` share the same non-layout selected-state style.
  - Replaced selected-state `width`, `marginLeft`, and `borderLeft` changes with an inset `boxShadow`, preserving the visual left bar without changing row width or horizontal position.
  - Added `tests/proxy-selected-style.test.ts` and widened `pnpm test` to run all `tests/*.test.ts`.
  - Verification: `pnpm test` first failed because the helper did not exist, then passed with 4 tests after implementation.
- Completed first-paint and connection white-flash audit:
  - The connection DataGrid background sequence (`60168dd3`, `76015e3a`, `8bea609e`) is not replayed because it already fixed dark-mode remount white flash, then adjusted light-mode card color, then reverted the DataGrid background overrides to restore rounded corners.
  - Current recovery branch intentionally stays at the post-revert `v1.7.7`-compatible `connection-table.tsx` surface for this area.
  - The first-paint external CSS idea from `fdbb916d` remains a separate redo candidate, but it is not replayed here because the original release-note context bundles it with excluded warm-to-tray/window lifecycle changes.
  - Any first-paint background redo must be verified in an actual Tauri window for CSP behavior, MUI emotion runtime styles, light/dark first paint, rounded corners, and no white flash before it can be kept.

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

- Do not replay this group into the recovery branch unless classic Clash compatibility is explicitly chosen as product policy.
- Current recovery branch intentionally keeps `v1.7.7` defaults: ports `7895/7896/7897/7898/7899`, controller `9097`, latency fallback `10000`, and placeholder `http://1.1.1.1`.
- The latency placeholder cleanup can be reconsidered separately only if it has a narrow UI requirement and does not bundle timeout/default-port changes.

#### Profile Extension Cards Removal

Related commit:

- `5d192a02` `refactor(profiles): remove global extensions cards`

Evidence:

- Removes `ProfileMore` import and the Merge/Script cards from `src/pages/profiles.tsx`.
- Removes the `getRuntimeLogs` SWR hook and several `mutateLogs()` calls that were only used by those cards.

Risk:

- This is a feature removal, not a bug fix.
- The removed cards are part of the profile editing surface; removing them can strand users who still need Merge/Script extension editing.
- Later profile switch rewrites are built on top of this smaller profile page, so blindly replaying it increases conflict risk.

Recommendation:

- Do not replay into the recovery branch unless the product decision is to remove global profile extension editing.
- If the cards cause a specific bug, reproduce that bug first and fix the cards directly instead of deleting the feature.

#### Dependency and Win32 Surface Expansion

Related commits:

- `16357461` `chore(deps): bump meta-json-schema 1.18.6 -> 1.19.24`
- `e2c46caa` adds the `windows-sys` dependency for direct Win32 window-style mutation.

Evidence:

- `meta-json-schema@1.19.24` adds an engine declaration requiring Node `>=18` and pnpm `>=9`.
- `windows-sys` is only used by the warm-to-tray/taskbar bypass implementation.

Risk:

- Dependency bumps change the support matrix and can introduce build differences unrelated to the cleanup.
- The `windows-sys` dependency has no purpose without the excluded Win32 window lifecycle changes.

Recommendation:

- Do not replay dependency bumps as part of cleanup.
- Only update `meta-json-schema` in a separate dependency-maintenance branch with lockfile verification.
- Keep `windows-sys` excluded while warm-to-tray and taskbar bypass remain excluded.

#### Consolidated Hot-Path Hardening Commit

Related commit:

- `44ec26c8` `fix(legacy): harden post-.13 hot paths against silent degradations`

Evidence:

- Bundles seven areas in one commit: latency field clearing, PATCH timeout, selector restoration retry, profile import switching, async config validation warning, connection table background, and warning notices.
- Some pieces depend on earlier high-risk changes such as async `check_config`, single `PUT /configs`, and profile optimistic switching.

Risk:

- The commit mixes unrelated frontend, backend, profile, and core behavior changes.
- Some sub-fixes are valid candidates, but replaying the combined commit would also reintroduce excluded runtime behavior.

Recommendation:

- Do not replay this commit as-is.
- Split into separate redo candidates only if each has a focused repro:
  - clearing latency fields in Misc settings;
  - shorter PATCH timeout for `stop_core`;
  - selector restore retry window;
  - connection table color alignment.
- Keep async config validation and profile import switching in the high-risk redo bucket until profile/core flows have tests.

### Likely Keep

These changes look low-risk or narrowly scoped, but still need normal build checks in the clean branch.

- Release notes in `UPDATELOG.md`.
- `CLAUDE.md` project guidelines.
- `i18n(tray): rename "Restart Clash" to "Restart Core"`.
- `fix(logs): persist log level filter across page navigation`.
- `fix(notice): unify info variant layout with success/error`.
- Connection/proxy visual jitter fixes only if visual QA confirms they do not regress layout.
- `fix: pin legacy service binaries`, but preferably as part of a smaller build-script commit.

### Optional Small Redo Candidates

These are not currently replayed. They look narrower than the runtime rewrites, but should still be reintroduced one at a time with a fresh check.

#### Connection Sorting

Related commit:

- `88c789f3` `fix: improve connection speed sorting`

Recommendation:

- Replayed as a separate recovery commit instead of cherry-picking the original messy commit.
- Verification now uses `pnpm test`, which covers default time sorting, upload/download speed sorting, totals, non-mutating selection, and descending-first DataGrid numeric column sorting.

#### First-Paint Background and Connection Page White Flash

Related commits:

- `60168dd3` `fix(connections): cover DataGrid container bg to avoid white flash on remount`
- `76015e3a` `fix(connections): restore white paper card by aligning DataGrid bg with outer Box`
- `8bea609e` `revert(connections): restore rounded corners by reverting DataGrid bg overrides`
- `c6a56c60` and `fdbb916d` first-paint background attempts.

Recommendation:

- Do not replay the connection DataGrid background sequence as-is because it already contains a fix and revert cycle.
- The final state of that sequence (`8bea609e`) intentionally removed the DataGrid root and inner-container background overrides because they covered the outer rounded card corners. Recovery keeps that safer state.
- If white flash remains important, redo with visual QA in dark and light mode, including rounded-corner checks.
- Do not replay `c6a56c60` because inline `<style>` in `src/index.html` triggered Tauri 1.x CSP nonce mode and broke emotion/MUI runtime styles in the follow-up diagnosis.
- The external CSS first-paint background from `fdbb916d` can be considered separately because it avoids inline CSP nonce issues, but it must be rebuilt as its own change without the warm-to-tray release-note context and with a Tauri window smoke test.

#### Proxy List Jitter

Related commits:

- `32edf90c` `fix(proxies): stabilize node list initial layout to prevent jitter`
- `ae8dc76f` `fix(proxies): replace selected-row width shift with non-layout boxShadow`

Recommendation:

- Replayed only the selected-row width-shift portion from `ae8dc76f`, rebuilt as a shared helper with a regression test.
- The selected-row fix is accepted because the old style changed layout metrics (`width`, `marginLeft`, `borderLeft`) whenever `selected` toggled, while the new inset `boxShadow` does not change row geometry.
- Do not replay the rest of `32edf90c` yet:
  - `useLayoutEffect` hydration for stored group state still needs UI-level evidence that it does not introduce blocking work before paint.
  - `ResizeObserver(document.body)` still needs browser/Tauri checks for tray restore, window resize, and cleanup behavior.
  - fixed `img height=32` for group icons still needs visual QA for remote, data, and inline SVG icons.
- Remaining proxy list layout work should be verified with collapsed/open groups, multi-column mode, selected rows, custom group icons, and tray restore before any additional replay.

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
