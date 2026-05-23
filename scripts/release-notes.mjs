import fs from "fs-extra";
import path from "path";
import { resolveUpdateLog } from "./updatelog.mjs";

// Usage: node scripts/release-notes.mjs <tag> [--out <file>]
// Writes the matching UPDATELOG.md section to <file> (default: release_notes.md).
// Tag matching: tries exact tag first, then strips a `-legacy.rN` / `-rc.N` /
// `-beta.N` / `-alpha.N` suffix and tries the base version. Useful for fork
// builds that share an upstream UPDATELOG entry (e.g. v1.7.7-legacy.r1 → v1.7.7).
async function main() {
  const args = process.argv.slice(2);
  if (args.length === 0) {
    console.error("usage: release-notes.mjs <tag> [--out <file>]");
    process.exit(1);
  }

  const tag = args[0].startsWith("v") ? args[0] : `v${args[0]}`;
  const outIdx = args.indexOf("--out");
  const outFile = outIdx >= 0 ? args[outIdx + 1] : "release_notes.md";

  const baseTag = tag.replace(/-(?:legacy|rc|beta|alpha)(?:\.[\w]+)*$/, "");

  let body;
  let matched = tag;
  try {
    body = await resolveUpdateLog(tag);
  } catch (e) {
    if (baseTag !== tag) {
      try {
        body = await resolveUpdateLog(baseTag);
        matched = baseTag;
      } catch {
        throw e;
      }
    } else {
      throw e;
    }
  }

  const header =
    matched === tag
      ? body
      : `> Based on upstream **${matched}** notes.\n\n${body}`;

  await fs.writeFile(path.resolve(outFile), header + "\n");
  console.log(`wrote ${outFile} (matched ${matched})`);
}

main().catch((e) => {
  console.error(e.message || e);
  process.exit(1);
});
