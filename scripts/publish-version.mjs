// scripts/publish-version.mjs
import { execFileSync, spawn } from "child_process";
import { existsSync, readFileSync } from "fs";
import path from "path";

const rootDir = process.cwd();
const scriptPath = path.join(rootDir, "scripts", "release-version.mjs");
const releaseFiles = [
  "Changelog.md",
  "package.json",
  "src-tauri/Cargo.toml",
  "src-tauri/tauri.conf.json",
];

if (!existsSync(scriptPath)) {
  console.error("release-version.mjs not found!");
  process.exit(1);
}

const versionArg = process.argv[2];
if (!versionArg) {
  console.error("Usage: pnpm publish-version <version>");
  process.exit(1);
}

const runRelease = () =>
  new Promise((resolve, reject) => {
    const child = spawn("node", [scriptPath, versionArg], { stdio: "inherit" });
    child.on("exit", (code) => {
      if (code === 0) resolve();
      else reject(new Error("release-version failed"));
    });
  });

function isSemver(version) {
  return /^v?\d+\.\d+\.\d+(-[0-9A-Za-z-.]+)?$/.test(version);
}

function runGit(args, { captureOutput = false } = {}) {
  return execFileSync("git", args, {
    cwd: rootDir,
    encoding: "utf8",
    stdio: captureOutput ? "pipe" : "inherit",
  });
}

function readPackageVersion() {
  const packageJsonPath = path.join(rootDir, "package.json");
  const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
  return packageJson.version;
}

function getCurrentBranch() {
  return runGit(["rev-parse", "--abbrev-ref", "HEAD"], {
    captureOutput: true,
  }).trim();
}

function getChangedReleaseFiles() {
  const output = runGit(["diff", "--name-only", "HEAD", "--", ...releaseFiles], {
    captureOutput: true,
  }).trim();

  if (!output) {
    return [];
  }

  return output
    .split(/\r?\n/)
    .map((file) => file.trim())
    .filter(Boolean);
}

async function run() {
  await runRelease();

  const packageVersion = readPackageVersion();
  let tag = null;

  if (["alpha", "beta", "rc"].includes(versionArg)) {
    tag = `v${packageVersion}`;
  } else if (isSemver(versionArg)) {
    tag = versionArg.startsWith("v") ? versionArg : `v${versionArg}`;
  }

  if (!tag) {
    console.log("[INFO]: No git tag created for this version.");
    return;
  }

  try {
    const changedReleaseFiles = getChangedReleaseFiles();

    if (changedReleaseFiles.length > 0) {
      runGit(["add", "--", ...changedReleaseFiles]);
      runGit(["commit", "-m", `release: ${tag}`, "--", ...changedReleaseFiles]);
      console.log(
        `[INFO]: Created release commit with ${changedReleaseFiles.join(", ")}.`,
      );
    } else {
      console.log("[INFO]: No tracked release file changes detected, tagging HEAD.");
    }

    const branch = getCurrentBranch();
    runGit(["tag", tag]);
    runGit(["push", "origin", branch, tag]);
    console.log(`[INFO]: Pushed ${branch} and ${tag}.`);
  } catch {
    console.error(`[ERROR]: Failed to publish release tag: ${tag}`);
    process.exit(1);
  }
}

run();
