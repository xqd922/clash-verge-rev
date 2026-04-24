import { resolveUpdateLog } from "./updatelog.mjs";

const tag = process.argv[2];

if (!tag) {
  throw new Error("tag is required");
}

resolveUpdateLog(tag)
  .then((notes) => {
    process.stdout.write(notes);
  })
  .catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
