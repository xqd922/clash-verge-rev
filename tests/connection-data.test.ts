import test from "node:test";
import assert from "node:assert/strict";

import {
  NUMERIC_CONNECTION_SORTING_ORDER,
  selectConnections,
} from "../src/components/connection/connection-data.ts";

const connection = (
  id: string,
  patch: Partial<IConnectionsItem>
): IConnectionsItem => ({
  id,
  metadata: {
    network: "tcp",
    type: "HTTP",
    host: "",
    sourceIP: "127.0.0.1",
    sourcePort: "50000",
    destinationPort: "443",
    destinationIP: "1.1.1.1",
    process: "",
    processPath: "",
  },
  upload: 0,
  download: 0,
  start: "2026-01-01T00:00:00.000Z",
  chains: [],
  rule: "MATCH",
  rulePayload: "",
  ...patch,
});

test("selectConnections sorts by default time newest first and keeps totals", () => {
  const input = [
    connection("old", {
      upload: 10,
      download: 100,
      start: "2026-01-01T00:00:00.000Z",
    }),
    connection("new", {
      upload: 20,
      download: 200,
      start: "2026-01-02T00:00:00.000Z",
    }),
  ];

  const result = selectConnections(input, () => true, "Default");

  assert.deepEqual(
    result.connections.map((conn) => conn.id),
    ["new", "old"]
  );
  assert.equal(result.upload, 30);
  assert.equal(result.download, 300);
});

test("selectConnections sorts speed descending without mutating source order", () => {
  const input = [
    connection("slow", { curUpload: 1, curDownload: 3 }),
    connection("fast", { curUpload: 9, curDownload: 8 }),
    connection("middle", { curUpload: 5, curDownload: 13 }),
  ];
  const originalOrder = input.map((conn) => conn.id);

  const byUpload = selectConnections(input, () => true, "Upload Speed");
  const byDownload = selectConnections(input, () => true, "Download Speed");

  assert.deepEqual(
    byUpload.connections.map((conn) => conn.id),
    ["fast", "middle", "slow"]
  );
  assert.deepEqual(
    byDownload.connections.map((conn) => conn.id),
    ["middle", "fast", "slow"]
  );
  assert.deepEqual(
    input.map((conn) => conn.id),
    originalOrder
  );
  assert.notStrictEqual(byUpload.connections, input);
});

test("numeric connection table columns sort descending first", () => {
  assert.deepEqual(NUMERIC_CONNECTION_SORTING_ORDER, ["desc", "asc", null]);
});
