# Upstream Main Merge Guide

This repository syncs the original project from `upstream/main` into the
long-lived local branch `personal`.

The fixed policy for this repo is:

- Always use `merge`
- Never use `rebase`
- Always resolve upstream integration work on a dedicated sync branch first

## Why This Repo Uses `merge`

`personal` contains long-lived product differences, packaging changes, local
plugin paths, and release/version decisions. Rebase would rewrite those
integration points and make later syncs harder to review. Merge keeps the
history honest and lets `git rerere` learn repeated conflict resolutions.

## One-Time Setup

Run these once on a machine that will handle upstream syncs:

```powershell
git remote add upstream https://github.com/clash-verge-rev/clash-verge-rev.git
git config rerere.enabled true
git config merge.conflictstyle zdiff3
```

If `upstream` already exists, do not add it again. Just refresh it:

```powershell
git fetch upstream --prune
```

## Standard Merge SOP

### 1. Start from `personal`

```powershell
git switch personal
git fetch origin --prune
git fetch upstream --prune
git pull --ff-only origin personal
git switch -c sync/upstream-main-YYYYMMDD
```

Example:

```powershell
git switch -c sync/upstream-main-20260409
```

### 2. Merge upstream into the sync branch

```powershell
git merge --no-ff upstream/main
```

Notes:

- Do not merge `upstream/main` directly into `personal`
- Do not rebase `personal` onto `upstream/main`
- Resolve all conflicts on the sync branch first

### 3. Resolve conflicts with repo-specific rules

When upstream and `personal` diverge, keep these local decisions unless there
is a deliberate product change:

- Keep `personal` route structure and do not restore the removed `home` page
- Keep `personal` version numbers in `package.json` and `src-tauri/Cargo.toml`
- Keep local path dependencies for `tauri-plugin-mihomo` and
  `tauri-plugin-mihomo-api`
- Keep intentional `personal` behavior for the proxy/root page flow and other
  product customizations
- Accept new upstream dependencies when merged code requires them
- Regenerate generated files instead of hand-editing conflict markers
- Regenerate lockfiles instead of manually merging lockfile conflicts

### 4. Regenerate generated files and lockfiles

After conflicts are resolved, run the regeneration steps required by the merged
changes. Common commands for this repository are:

```powershell
pnpm i18n:types
cargo generate-lockfile
pnpm install --lockfile-only --ignore-scripts --no-frozen-lockfile
pnpm install --ignore-scripts --no-frozen-lockfile
```

If upstream added new packages that are now referenced by merged files, add
them explicitly and refresh the lockfile before the final install.

### 5. Validate the merge

Preferred validation set:

```powershell
pnpm lint
pnpm typecheck
cargo test --workspace
```

If `cargo test --workspace` fails in an upstream-owned test target that is not
caused by the merge resolution itself, run a narrower application validation to
confirm the merged app still builds correctly:

```powershell
cargo check -p clash-verge
cargo test -p clash-verge --lib
```

Use that narrower validation only as a fallback, and record the exact failing
workspace tests in the merge notes.

### 6. Review the result

Before committing, check the merge result carefully:

```powershell
git status
git diff --check
git diff --stat
```

If specific files are known conflict hotspots, inspect them directly before
committing.

### 7. Commit the sync branch

```powershell
git add -A
git commit
```

Recommended commit message pattern:

```text
merge: sync upstream/main YYYY-MM-DD
```

### 8. Merge the validated sync branch back into `personal`

```powershell
git switch personal
git merge --no-ff sync/upstream-main-YYYYMMDD
git push origin personal
```

## Exact Commands Used For The 2026-04-09 Merge

This section records the actual merge work performed for
`sync/upstream-main-20260409`.

### Branch and merge

```powershell
git remote add upstream https://github.com/clash-verge-rev/clash-verge-rev.git
git config rerere.enabled true
git config merge.conflictstyle zdiff3
git fetch upstream --prune
git switch personal
git switch -c sync/upstream-main-20260409
git merge --no-ff upstream/main
```

### Post-merge regeneration and dependency refresh

```powershell
pnpm i18n:types
cargo generate-lockfile
pnpm install --lockfile-only --ignore-scripts --no-frozen-lockfile
pnpm install --ignore-scripts --no-frozen-lockfile
```

Additional packages added because merged upstream code now references them:

```powershell
pnpm add validator@^13.15.26 --save-prod --lockfile-only
pnpm add @types/validator@^13.15.10 --save-dev --lockfile-only
pnpm add eslint-config-prettier --save-dev --lockfile-only
pnpm add eslint-plugin-prettier --save-dev --lockfile-only
```

Formatting used during conflict cleanup:

```powershell
pnpm exec prettier --write src eslint.config.ts package.json docs/upstream-main-merge.md
rustfmt src-tauri/src/cmd/profile.rs
```

### Merge decisions taken on 2026-04-09

- Kept the `personal` product structure without `src/pages/home.tsx`
- Kept `personal` version `7.0.14` in `package.json`
- Kept `personal` version `7.0.14` in `src-tauri/Cargo.toml`
- Kept the local plugin path dependency in `package.json`
- Kept the local plugin path dependency in `src-tauri/Cargo.toml`
- Adapted `src-tauri/src/cmd/profile.rs` to the current upstream Rust APIs
  after the merge
- Regenerated i18n generated types instead of hand-merging generated files
- Refreshed both Cargo and pnpm lockfiles instead of manual lockfile editing

### Validation results for the 2026-04-09 merge

Commands run:

```powershell
pnpm lint
pnpm typecheck
cargo check -p clash-verge
cargo test -p clash-verge --lib
cargo test --workspace
```

Observed results:

- `pnpm lint`: passed
- `pnpm typecheck`: passed
- `cargo check -p clash-verge`: passed
- `cargo test -p clash-verge --lib`: passed
- `cargo test --workspace`: failed in `tauri-plugin-mihomo` tests

Workspace test failures observed on 2026-04-09:

- `restart`
- `reload_config`
- `mihomo_common_flush_dns`
- `mihomo_common_flush_fakeip`
- `mihomo_common_get_version`
- `mihomo_common_patch_base_config`
- `mihomo_common_update_geo`

The failure signature was:

```text
Cannot start a runtime from within a runtime
```

That failure came from `tauri-plugin-mihomo` test targets rather than the app
merge resolution in `src-tauri`.

## Practical Notes For Every Future Merge

- Keep using the same merge-only strategy every time
- Let `rerere` learn repeated conflict resolutions
- Keep local customizations concentrated where possible so upstream syncs stay
  reviewable
- If a future upstream change replaces one of the local product decisions
  intentionally, update this document at the same time as the merge
