import fetch from "node-fetch";
import { getOctokit, context } from "@actions/github";
import { resolveUpdateLog } from "./updatelog.mjs";

const UPDATE_TAG_NAME = "updater-legacy";
const UPDATE_JSON_FILE = "update.json";
const UPDATE_JSON_PROXY = "update-proxy.json";
const RELEASE_TAG_PREFIX = "v";
const LEGACY_TAG_MARKER = "-legacy.";

async function getOrCreateRelease(github, options, tagName, name) {
  try {
    const { data } = await github.rest.repos.getReleaseByTag({
      ...options,
      tag: tagName,
    });
    return data;
  } catch (error) {
    if (error.status !== 404) {
      throw error;
    }
  }

  const { data } = await github.rest.repos.createRelease({
    ...options,
    tag_name: tagName,
    name,
    body: "Legacy updater assets",
    draft: false,
    prerelease: false,
  });

  return data;
}

async function resolveUpdater() {
  if (process.env.GITHUB_TOKEN === undefined) {
    throw new Error("GITHUB_TOKEN is required");
  }

  const options = { owner: context.repo.owner, repo: context.repo.repo };
  const github = getOctokit(process.env.GITHUB_TOKEN);

  const { data: tags } = await github.rest.repos.listTags({
    ...options,
    per_page: 20,
    page: 1,
  });

  const tag = tags.find(
    (item) =>
      item.name.startsWith(RELEASE_TAG_PREFIX) &&
      item.name.includes(LEGACY_TAG_MARKER)
  );

  if (!tag) {
    throw new Error("could not find a legacy release tag");
  }

  const { data: latestRelease } = await github.rest.repos.getReleaseByTag({
    ...options,
    tag: tag.name,
  });

  const updateData = {
    name: tag.name,
    notes: await resolveUpdateLog(tag.name),
    pub_date: new Date().toISOString(),
    platforms: {
      win64: { signature: "", url: "" },
      linux: { signature: "", url: "" },
      darwin: { signature: "", url: "" },
      "darwin-aarch64": { signature: "", url: "" },
      "darwin-intel": { signature: "", url: "" },
      "darwin-x86_64": { signature: "", url: "" },
      "linux-x86_64": { signature: "", url: "" },
      "linux-x86": { signature: "", url: "" },
      "linux-i686": { signature: "", url: "" },
      "linux-aarch64": { signature: "", url: "" },
      "linux-armv7": { signature: "", url: "" },
      "windows-x86_64": { signature: "", url: "" },
      "windows-aarch64": { signature: "", url: "" },
      "windows-x86": { signature: "", url: "" },
      "windows-i686": { signature: "", url: "" },
    },
  };

  const promises = latestRelease.assets.map(async (asset) => {
    const { name, browser_download_url } = asset;

    if (name.endsWith("x64-setup.nsis.zip")) {
      updateData.platforms.win64.url = browser_download_url;
      updateData.platforms["windows-x86_64"].url = browser_download_url;
    }
    if (name.endsWith("x64-setup.nsis.zip.sig")) {
      const sig = await getSignature(browser_download_url);
      updateData.platforms.win64.signature = sig;
      updateData.platforms["windows-x86_64"].signature = sig;
    }

    if (name.endsWith("x86-setup.nsis.zip")) {
      updateData.platforms["windows-x86"].url = browser_download_url;
      updateData.platforms["windows-i686"].url = browser_download_url;
    }
    if (name.endsWith("x86-setup.nsis.zip.sig")) {
      const sig = await getSignature(browser_download_url);
      updateData.platforms["windows-x86"].signature = sig;
      updateData.platforms["windows-i686"].signature = sig;
    }

    if (name.endsWith("arm64-setup.nsis.zip")) {
      updateData.platforms["windows-aarch64"].url = browser_download_url;
    }
    if (name.endsWith("arm64-setup.nsis.zip.sig")) {
      const sig = await getSignature(browser_download_url);
      updateData.platforms["windows-aarch64"].signature = sig;
    }

    if (name.endsWith(".app.tar.gz") && !name.includes("aarch")) {
      updateData.platforms.darwin.url = browser_download_url;
      updateData.platforms["darwin-intel"].url = browser_download_url;
      updateData.platforms["darwin-x86_64"].url = browser_download_url;
    }
    if (name.endsWith(".app.tar.gz.sig") && !name.includes("aarch")) {
      const sig = await getSignature(browser_download_url);
      updateData.platforms.darwin.signature = sig;
      updateData.platforms["darwin-intel"].signature = sig;
      updateData.platforms["darwin-x86_64"].signature = sig;
    }

    if (name.endsWith("aarch64.app.tar.gz")) {
      updateData.platforms["darwin-aarch64"].url = browser_download_url;
      updateData.platforms.linux.url = browser_download_url;
      updateData.platforms["linux-x86_64"].url = browser_download_url;
      updateData.platforms["linux-x86"].url = browser_download_url;
      updateData.platforms["linux-i686"].url = browser_download_url;
      updateData.platforms["linux-aarch64"].url = browser_download_url;
      updateData.platforms["linux-armv7"].url = browser_download_url;
    }
    if (name.endsWith("aarch64.app.tar.gz.sig")) {
      const sig = await getSignature(browser_download_url);
      updateData.platforms["darwin-aarch64"].signature = sig;
      updateData.platforms.linux.signature = sig;
      updateData.platforms["linux-x86_64"].signature = sig;
      updateData.platforms["linux-aarch64"].signature = sig;
      updateData.platforms["linux-armv7"].signature = sig;
    }
  });

  await Promise.allSettled(promises);

  Object.entries(updateData.platforms).forEach(([key, value]) => {
    if (!value.url) {
      delete updateData.platforms[key];
    }
  });

  const updateDataProxy = JSON.parse(JSON.stringify(updateData));
  Object.entries(updateDataProxy.platforms).forEach(([, value]) => {
    if (value.url) {
      value.url = `https://mirror.ghproxy.com/${value.url}`;
    }
  });

  const updateRelease = await getOrCreateRelease(
    github,
    options,
    UPDATE_TAG_NAME,
    "Legacy updater assets"
  );

  for (const asset of updateRelease.assets) {
    if (asset.name === UPDATE_JSON_FILE || asset.name === UPDATE_JSON_PROXY) {
      await github.rest.repos
        .deleteReleaseAsset({ ...options, asset_id: asset.id })
        .catch(console.error);
    }
  }

  await github.rest.repos.uploadReleaseAsset({
    ...options,
    release_id: updateRelease.id,
    name: UPDATE_JSON_FILE,
    data: JSON.stringify(updateData, null, 2),
  });

  await github.rest.repos.uploadReleaseAsset({
    ...options,
    release_id: updateRelease.id,
    name: UPDATE_JSON_PROXY,
    data: JSON.stringify(updateDataProxy, null, 2),
  });
}

async function getSignature(url) {
  const response = await fetch(url, {
    method: "GET",
    headers: { "Content-Type": "application/octet-stream" },
  });

  return response.text();
}

resolveUpdater().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
