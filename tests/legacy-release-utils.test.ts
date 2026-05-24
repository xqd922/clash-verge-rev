import test from "node:test";
import assert from "node:assert/strict";

import { shouldRunPreTauriCheck } from "../scripts/build-options.mjs";
import { resolveLegacyReleaseTag } from "../scripts/legacy-release-utils.mjs";

test("resolveLegacyReleaseTag prefers the workflow release tag", () => {
  const tag = resolveLegacyReleaseTag(
    [{ name: "v1.7.7-legacy.20" }, { name: "v1.7.7-legacy.r1" }],
    "v1.7.7-legacy.22"
  );

  assert.equal(tag.name, "v1.7.7-legacy.22");
});

test("resolveLegacyReleaseTag ignores r-series tags when selecting numbered legacy tags", () => {
  const tag = resolveLegacyReleaseTag([
    { name: "v1.7.7-legacy.r1" },
    { name: "v1.7.7-legacy.20" },
    { name: "v1.7.7-legacy.22" },
    { name: "v1.7.7-legacy.9" },
  ]);

  assert.equal(tag.name, "v1.7.7-legacy.22");
});

test("shouldRunPreTauriCheck can skip the redundant workflow build check", () => {
  assert.equal(shouldRunPreTauriCheck({}), true);
  assert.equal(shouldRunPreTauriCheck({ SKIP_PRE_TAURI_CHECK: "true" }), false);
});
