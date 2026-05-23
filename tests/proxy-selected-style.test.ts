import test from "node:test";
import assert from "node:assert/strict";

import { getSelectedProxyItemSx } from "../src/components/proxy/proxy-selected-style.ts";

test("selected proxy style does not change item layout metrics", () => {
  const style = getSelectedProxyItemSx("#1677ff", "rgba(22, 119, 255, 0.15)");

  assert.equal(style.width, undefined);
  assert.equal(style.marginLeft, undefined);
  assert.equal(style.borderLeft, undefined);
  assert.equal(style.boxShadow, "inset 3px 0 0 #1677ff");
  assert.equal(style.bgcolor, "rgba(22, 119, 255, 0.15)");
});
