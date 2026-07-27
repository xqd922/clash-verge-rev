# Upstream v2.5.2 Selective Adoption

## Scope and Baselines

- Upstream target: `v2.5.2` at `28f2efc5`.
- Personal baseline: `personal` at `36f771c2`.
- Product version remains `7.0.34`.
- Changes are ported by subsystem. The upstream tree and history are not
  merged wholesale.
- Functional and reliability improvements may be adopted, while the personal
  visual identity and product structure remain authoritative.

## Adopted Upstream Capabilities

- Mihomo plugin 0.5.4 Rust and JavaScript APIs, permissions, generated types,
  and Smart weights/cache commands. Unknown proxy types remain compatible so
  Smart groups can round-trip through the API.
- Shared WebSocket subscriptions, visibility-aware suspension, cleanup,
  cumulative traffic fields, and bounded connection/log history.
- Serialized core start/stop/restart, startup readiness checks, rollback,
  portable sidecar behavior, Windows service coordination, and sidecar to
  service handoff.
- Profile switch busy gating, `ValidationOutcome` propagation, rollback after
  validation failure or timeout, and a latest-wins frontend queue. Connections
  are closed and success is reported only after a valid switch.
- Asynchronous selected-node restoration with generation guards, a confirming
  second snapshot, stale-record cleanup, and Smart group unfixing.
- Smart `Model.bin` preparation only for the Smart core, with source and copy
  length checks plus a staged replacement in the destination directory.
- Profile path validation, YAML emission fixes, network and URI parser fixes,
  and safer build-script process invocation.
- Consistent local/WebDAV backup snapshots, staged restore and rollback,
  archive validation, bounded extraction, and safer backup file handling.
- Refreshed Cargo and pnpm locks plus local plugin 0.5.4 integration.

## Adopted With Personal Adaptation

- The profile state machine is integrated into the existing personal Profile
  page instead of adopting the upstream page structure.
- Smart selected-node restoration preserves the personal behavior where Smart
  automation is immediately unfixed after restoring a node.
- Traffic, connection, profile, and editor components receive data and error
  handling improvements without visual restructuring.
- Smart model and core-service behavior is hardened for the personal Smart
  core and portable-mode workflow.

## Reliability and Safety Hardening

- Core lifecycle, service install/uninstall, and sidecar-to-service handoff
  share the configuration permit and lifecycle lock. Sidecars are tracked by
  PID and generation, stale exit events cannot overwrite current state, and
  failed handoffs stop residual processes.
- Configuration writes use same-directory staging, flush/sync, and atomic
  replacement. Failed core changes restore the last committed `verge.yaml`.
- Profile metadata accepts only safe single-component filenames. Switching is
  latest-wins with validation and rollback, invalid reorder IDs preserve the
  list, and deletion retains shared Merge/Script dependencies.
- Backup capture runs under the Profile transaction lock and configuration
  permit. Restore uses a staging directory and transaction rollback, removes
  stale DNS when the archive omits it, rejects traversal and special files,
  and refreshes timers only after commit.
- ZIP restore limits are 2,048 entries, 128 MiB per entry, 512 MiB total
  uncompressed data, 32 MiB per critical configuration, and a 200:1 maximum
  compression ratio.
- WebDAV clients are generation-bound so an old asynchronous initialization
  cannot repopulate the cache after credentials change. Downloads stream into
  exclusive temporary files with cancellation cleanup and a 544 MiB transfer
  cap; listing responses are capped at 8 MiB.
- Local export stages in the destination directory, flushes and syncs the
  complete copy, then atomically replaces the destination. It does not
  truncate the source or an existing destination on failure.

## Explicitly Skipped

- Upstream fonts, font stacks, theme defaults, and font-loading behavior.
- Upstream page layouts, card appearance, Proxy/Profile visuals, titlebar, and
  navigation redesigns.
- Home-page or root-route changes that would replace the personal Proxy route.
- Upstream application version and branding overrides.
- Replacing the local Mihomo plugin path dependency with an external package.
- Removing `node-fetch`, which remains paired with `https-proxy-agent` for the
  personal prebuild and updater workflows.
- A direct merge or rebase of the full upstream history.

## Preserved Personal Design

- The existing style, theme, typography, and custom-font system.
- Existing routes, navigation, 36 px titlebar, Windows caption controls,
  Profile/Proxy cards, and interaction density.
- Smart core selection, conversion, weights, ranking, updater, and release
  workflow.
- The `7.0.34` version line, local plugin path, and personal packaging rules.
- Upstream behavior patches must fit inside existing components without
  changing their visual hierarchy.

## Regression Protection

- `pnpm ui:check` protects the personal UI, font, route, and titlebar contract.
- Frontend lint, type checking, and production build are required.
- Rust formatting, clippy, workspace tests, and Mihomo plugin tests are
  required before the adoption is committed.

## Validation Matrix

- Personal UI regression checks, frontend lint, type checking, and production
  web build.
- Rust formatting and whitespace checks.
- Strict application and all-target/all-feature Clippy checks.
- Application library/all-target checks and hermetic workspace/plugin tests.
  The live named-pipe `common_test` passes serially (8/8); provider integration
  tests additionally require the running Mihomo configuration to contain the
  test provider and are therefore environment-dependent.
- Focused regression tests for Profile integrity, lifecycle/config permits,
  atomic YAML writes, backup archive/rollback behavior, WebDAV cache
  generations, response limits, partial cleanup, and atomic local export.
- Browser verification of Proxy, Profile, and Settings at 1024 x 720 and
  1440 x 900 in light and dark themes, with the personal layout and fonts
  preserved.
