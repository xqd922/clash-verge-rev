# Rebuild 1.x Legacy Branch Design

## Context

The current working branch was `release/1.7.x-legacy`, at commit `7d2b1875`
(`Release 1.7.7`). The existing remote branch `origin/release/1.x-legacy`
is ahead of it by 66 commits and points at `84ce102e`
(`Release 1.7.7-legacy.21`).

The remote branch contains behavior and release work that matches the desired
legacy direction, but its history mixes release commits, reverted attempts,
workflow changes, scripts, UI fixes, and Windows behavior experiments. The new
long-term branch should keep the useful behavior while replacing the noisy
history with maintainable, reviewable commits.

## Goal

Create a clean replacement for `release/1.x-legacy` that becomes the long-term
1.x legacy maintenance branch.

The replacement branch should:

- preserve the useful behavior from `origin/release/1.x-legacy`;
- organize changes by functional area rather than old release sequence;
- make future rollback and review practical;
- avoid carrying forward temporary, reverted, or release-number-only noise;
- keep the current `release/1.7.x-legacy` baseline understandable.

## Branch Strategy

Create the rebuild branch from `release/1.7.x-legacy`, not from
`origin/release/1.x-legacy`.

Planned references:

- Source baseline: `release/1.7.x-legacy` at `7d2b1875`.
- Source of useful changes: `origin/release/1.x-legacy` at `84ce102e`.
- Working rebuild branch: `work/rebuild-1x-legacy`.
- Backup before replacement: `backup/release-1.x-legacy-before-rebuild`.
- Final replacement target: `release/1.x-legacy`.

The existing `origin/release/1.x-legacy` branch should be treated as a source
of behavior and patches, not as the branch to keep building on. Replacement of
the remote branch should be a final, explicit step after review and validation.

## Migration Groups

Migrate changes by functional area. Do not follow the old branch commit order.

### Release And Packaging

Bring over legacy release workflow, updater scripts, portable package scripts,
legacy artifact naming, and version handling that are needed for future
maintenance releases.

Clean up duplicated scripts, intermediate release-only changes, and temporary
logic that existed only to produce old `.legacy.N` artifacts.

### Window, Tray, Silent Start

Preserve the final desired Windows behavior:

- startup works normally;
- silent boot auto-launch remains scoped to boot auto-launch;
- close-to-tray behavior is stable;
- tray re-open behavior does not leave broken or unusable window states;
- WebView2 warm-up behavior is retained only where it is needed.

Intermediate implementations that were later reverted or superseded should not
be kept just because they exist in the old branch history.

### Tauri Backend Configuration

Bring over required legacy app identity, configuration isolation, directory
resolution, clash/verge compatibility, and related backend changes.

The rebuilt branch must not accidentally share runtime state with the non-legacy
application line when the old branch intended separation.

### Frontend Experience Fixes

Migrate user-visible stability and experience fixes in focused groups:

- profiles behavior and notices;
- proxy list layout stability;
- connections table background and remount behavior;
- log level persistence;
- first-paint theme handling;
- shared notice layout.

Style patches should be retained only when they support a concrete behavior or
visual stability goal.

### Documentation And Changelog

Update `README.md`, `CONTRIBUTING.md`, and `UPDATELOG.md` after the functional
migration is complete.

Do not copy the old `.legacy.1` through `.legacy.21` process history verbatim.
Summarize the new long-term branch behavior and release expectations clearly.

## Acceptance Standard

A change should be kept only when it serves the new long-term
`release/1.x-legacy` branch and can be explained through code review, automated
checks, or an explicit manual verification step.

## Verification Strategy

Validate each migration group before moving to the next one.

For every group:

- inspect the diff to confirm it is limited to the intended area;
- run the most relevant local checks available for that area;
- avoid stacking another migration group on top of an unresolved failure.

For the final branch:

- run the project check script, expected to be `pnpm check` if still current;
- run Rust/Tauri validation, at minimum `cargo check` in `src-tauri` if a full
  app build is too expensive;
- inspect release scripts and workflow diffs before replacing the branch;
- perform manual Windows checks for startup, silent boot auto-launch,
  close-to-tray, tray re-open, and WebView2 warm-up behavior.

## Replacement Guardrail

The branch replacement should be a separate final decision. The rebuild work can
prepare `work/rebuild-1x-legacy`, but updating the remote `release/1.x-legacy`
should happen only after the rebuilt branch is reviewed and validated.

Before replacement, create or confirm a backup reference for the old branch:
`backup/release-1.x-legacy-before-rebuild`.

## Non-Goals

- Preserve every old release commit.
- Preserve every old changelog entry exactly as written.
- Keep implementations that were reverted or superseded by later fixes.
- Redesign unrelated application architecture outside the legacy branch rebuild.
- Replace the remote branch before the rebuilt branch is validated.
