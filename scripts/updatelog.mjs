import fs from "fs-extra";
import path from "path";

const UPDATE_LOG = "UPDATELOG.md";
const LEGACY_RELEASE_RE = /^(v[0-9A-Za-z._-]+)-legacy\.[0-9]+$/;

function resolveUpdateLogTag(tag, map) {
  if (map[tag]) {
    return tag;
  }

  const match = tag.match(LEGACY_RELEASE_RE);
  if (match) {
    const rebuildTag = `${match[1]}-legacy-rebuild`;
    if (map[rebuildTag]) {
      return rebuildTag;
    }
  }

  throw new Error(`could not found "${tag}" in UPDATELOG.md`);
}

// parse the UPDATELOG.md
export async function resolveUpdateLog(tag) {
  const cwd = process.cwd();

  const reTitle = /^##\s+v[0-9A-Za-z._-]+/;
  const reEnd = /^---/;

  const file = path.join(cwd, UPDATE_LOG);

  if (!(await fs.pathExists(file))) {
    throw new Error("could not found UPDATELOG.md");
  }

  const data = await fs.readFile(file).then((d) => d.toString("utf8"));

  const map = {};
  let p = "";

  data.split("\n").forEach((line) => {
    if (reTitle.test(line)) {
      p = line.slice(3).trim();
      if (!map[p]) {
        map[p] = [];
      } else {
        throw new Error(`Tag ${p} dup`);
      }
    } else if (reEnd.test(line)) {
      p = "";
    } else if (p) {
      map[p].push(line);
    }
  });

  const resolvedTag = resolveUpdateLogTag(tag, map);

  return map[resolvedTag].join("\n").trim();
}
