import fs from "fs-extra";
import path from "path";
import AdmZip from "adm-zip";
import { createRequire } from "module";
import { getOctokit, context } from "@actions/github";

const APP_EXE_NAME = "Clash Verge Rev Legacy.exe";
const ZIP_PREFIX = "Clash.Verge.Rev.Legacy";
const target = process.argv.slice(2)[0];
const alpha = process.argv.slice(2)[1];

const ARCH_MAP = {
  "x86_64-pc-windows-msvc": "x64",
  "i686-pc-windows-msvc": "x86",
  "aarch64-pc-windows-msvc": "arm64",
};

const PROCESS_MAP = {
  x64: "x64",
  ia32: "x86",
  arm64: "arm64",
};

const arch = target ? ARCH_MAP[target] : PROCESS_MAP[process.arch];

async function resolvePortable() {
  if (process.platform !== "win32") return;

  const releaseDir = target
    ? `./src-tauri/target/${target}/release`
    : "./src-tauri/target/release";
  const configDir = path.join(releaseDir, ".config");

  if (!(await fs.pathExists(releaseDir))) {
    throw new Error("could not found the release dir");
  }

  await fs.ensureDir(configDir);
  await fs.createFile(path.join(configDir, "PORTABLE"));

  const zip = new AdmZip();
  zip.addLocalFile(path.join(releaseDir, APP_EXE_NAME));
  zip.addLocalFile(path.join(releaseDir, "verge-mihomo.exe"));
  zip.addLocalFile(path.join(releaseDir, "verge-mihomo-alpha.exe"));
  zip.addLocalFolder(path.join(releaseDir, "resources"), "resources");
  zip.addLocalFolder(configDir, ".config");

  const require = createRequire(import.meta.url);
  const packageJson = require("../package.json");
  const { version } = packageJson;

  const zipFile = `${ZIP_PREFIX}_${version}_${arch}_portable.zip`;
  zip.writeZip(zipFile);

  if (process.env.GITHUB_TOKEN === undefined) {
    throw new Error("GITHUB_TOKEN is required");
  }

  const options = { owner: context.repo.owner, repo: context.repo.repo };
  const github = getOctokit(process.env.GITHUB_TOKEN);
  const tag = alpha ? "alpha" : process.env.TAG_NAME || `v${version}`;

  const { data: release } = await github.rest.repos.getReleaseByTag({
    ...options,
    tag,
  });

  const assets = release.assets.filter((asset) => asset.name === zipFile);
  if (assets.length > 0) {
    await github.rest.repos.deleteReleaseAsset({
      ...options,
      asset_id: assets[0].id,
    });
  }

  await github.rest.repos.uploadReleaseAsset({
    ...options,
    release_id: release.id,
    name: zipFile,
    data: zip.toBuffer(),
  });
}

resolvePortable().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
