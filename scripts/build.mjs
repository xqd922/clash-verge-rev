import { spawnSync } from "child_process";

const rawArgs = process.argv.slice(2);
const PNPM_BIN = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
const SHELL_BIN = process.env.ComSpec || "cmd.exe";
const BUILD_ENV = {
  ...process.env,
  NODE_OPTIONS: withNodeHeap(process.env.NODE_OPTIONS),
};

function withNodeHeap(nodeOptions = "") {
  if (nodeOptions.includes("--max_old_space_size")) {
    return nodeOptions;
  }

  return `${nodeOptions} --max_old_space_size=4096`.trim();
}

function findTarget(args) {
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if ((arg === "--target" || arg === "-t") && args[i + 1]) {
      return args[i + 1];
    }
    if (arg.startsWith("--target=")) {
      return arg.slice("--target=".length);
    }
  }
  return null;
}

function run(args) {
  const result =
    process.platform === "win32"
      ? spawnSync(SHELL_BIN, ["/d", "/s", "/c", quoteCmd(PNPM_BIN, args)], {
          stdio: "inherit",
          env: BUILD_ENV,
        })
      : spawnSync(PNPM_BIN, args, {
          stdio: "inherit",
          env: BUILD_ENV,
        });

  if (result.error) {
    throw result.error;
  }

  if (typeof result.status === "number" && result.status !== 0) {
    process.exit(result.status);
  }

  if (result.signal) {
    process.exit(1);
  }
}

function quoteCmd(bin, args) {
  return [bin, ...args]
    .map((arg) => {
      if (/[\s"]/u.test(arg)) {
        return `"${arg.replaceAll('"', '\\"')}"`;
      }
      return arg;
    })
    .join(" ");
}

const target = findTarget(rawArgs);
const checkArgs = ["check"];

if (target) {
  checkArgs.push(target);
}

run(checkArgs);
run(["tauri", "build", ...rawArgs]);
