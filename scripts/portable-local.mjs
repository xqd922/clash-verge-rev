import fs from "fs";
import path from "path";
import AdmZip from "adm-zip";
import { createRequire } from "module";

const target = process.argv.slice(2)[0];

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

/// 本地打包便携版 (only Windows)
async function resolvePortable() {
  if (process.platform !== "win32") {
    console.log("[INFO]: Portable build is only supported on Windows");
    return;
  }

  const releaseDir = target
    ? `./src-tauri/target/${target}/release`
    : `./src-tauri/target/release`;
  const configDir = path.join(releaseDir, ".config");

  if (!fs.existsSync(releaseDir)) {
    throw new Error("Could not find the release directory: " + releaseDir);
  }

  // 创建配置目录
  if (!fs.existsSync(configDir)) {
    fs.mkdirSync(configDir, { recursive: true });
  }

  // 创建便携版标记文件
  const portableFile = path.join(configDir, "PORTABLE");
  if (!fs.existsSync(portableFile)) {
    fs.writeFileSync(portableFile, "");
  }

  const zip = new AdmZip();

  // 添加主程序
  const mainExe = path.join(releaseDir, "Clash Verge.exe");
  if (fs.existsSync(mainExe)) {
    zip.addLocalFile(mainExe);
  } else {
    console.warn("[WARN]: Main executable not found:", mainExe);
  }

  // 添加mihomo核心
  const mihomoExe = path.join(releaseDir, "verge-mihomo.exe");
  if (fs.existsSync(mihomoExe)) {
    zip.addLocalFile(mihomoExe);
  } else {
    console.warn("[WARN]: Mihomo executable not found:", mihomoExe);
  }

  // 添加mihomo alpha核心
  const mihomoAlphaExe = path.join(releaseDir, "verge-mihomo-alpha.exe");
  if (fs.existsSync(mihomoAlphaExe)) {
    zip.addLocalFile(mihomoAlphaExe);
  } else {
    console.warn("[WARN]: Mihomo alpha executable not found:", mihomoAlphaExe);
  }

  // 添加资源文件夹
  const resourcesDir = path.join(releaseDir, "resources");
  if (fs.existsSync(resourcesDir)) {
    zip.addLocalFolder(resourcesDir, "resources");
  } else {
    console.warn("[WARN]: Resources directory not found:", resourcesDir);
  }

  // 添加配置文件夹
  zip.addLocalFolder(configDir, ".config");

  // 获取版本号
  const require = createRequire(import.meta.url);
  const packageJson = require("../package.json");
  const { version } = packageJson;

  // 创建zip文件
  const zipFile = `Clash.Verge_${version}_${arch}_portable.zip`;
  zip.writeZip(zipFile);

  console.log(`[INFO]: Created portable zip successfully: ${zipFile}`);
  console.log(
    `[INFO]: File size: ${(fs.statSync(zipFile).size / 1024 / 1024).toFixed(2)} MB`,
  );

  return zipFile;
}

resolvePortable().catch(console.error);
