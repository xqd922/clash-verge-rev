import fetch from "node-fetch";
import { getOctokit, context } from "@actions/github";
import { resolveUpdateLog } from "./updatelog.mjs";

const UPDATE_TAG_NAME = "updater-legacy";
const UPDATE_JSON_FILE = "update-fixed-webview2.json";
const UPDATE_JSON_PROXY = "update-fixed-webview2-proxy.json";
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

function isLegacyReleaseTag(tagName) {
  return (
    tagName.startsWith(RELEASE_TAG_PREFIX) &&
    tagName.includes(LEGACY_TAG_MARKER)
  );
}

async function resolveReleaseTag(github, options) {
  const requestedTag = process.argv[2] || process.env.RELEASE_TAG;

  if (requestedTag) {
    if (!isLegacyReleaseTag(requestedTag)) {
      throw new Error(`legacy release tag expected, got "${requestedTag}"`);
    }
    return requestedTag;
  }

  const { data: tags } = await github.rest.repos.listTags({
    ...options,
    per_page: 20,
    page: 1,
  });

  const tag = tags.find((item) => isLegacyReleaseTag(item.name));

  if (!tag) {
    throw new Error("could not find a legacy release tag");
  }

  return tag.name;
}

async function resolveUpdater() {
  if (process.env.GITHUB_TOKEN === undefined) {
    throw new Error("GITHUB_TOKEN is required");
  }

  const options = { owner: context.repo.owner, repo: context.repo.repo };
  const github = getOctokit(process.env.GITHUB_TOKEN);

  const releaseTag = await resolveReleaseTag(github, options);

  const { data: latestRelease } = await github.rest.repos.getReleaseByTag({
    ...options,
    tag: releaseTag,
  });

  const updateData = {
    name: releaseTag,
    notes: await resolveUpdateLog(releaseTag),
    pub_date: new Date().toISOString(),
    platforms: {
      "windows-x86_64": { signature: "", url: "" },
      "windows-aarch64": { signature: "", url: "" },
      "windows-x86": { signature: "", url: "" },
      "windows-i686": { signature: "", url: "" },
    },
  };

  const promises = latestRelease.assets.map(async (asset) => {
    const { name, browser_download_url } = asset;

    if (name.endsWith("x64_fixed_webview2-setup.nsis.zip")) {
      updateData.platforms["windows-x86_64"].url = browser_download_url;
    }
    if (name.endsWith("x64_fixed_webview2-setup.nsis.zip.sig")) {
      const sig = await getSignature(browser_download_url);
      updateData.platforms["windows-x86_64"].signature = sig;
    }

    if (name.endsWith("x86_fixed_webview2-setup.nsis.zip")) {
      updateData.platforms["windows-x86"].url = browser_download_url;
      updateData.platforms["windows-i686"].url = browser_download_url;
    }
    if (name.endsWith("x86_fixed_webview2-setup.nsis.zip.sig")) {
      const sig = await getSignature(browser_download_url);
      updateData.platforms["windows-x86"].signature = sig;
      updateData.platforms["windows-i686"].signature = sig;
    }

    if (name.endsWith("arm64_fixed_webview2-setup.nsis.zip")) {
      updateData.platforms["windows-aarch64"].url = browser_download_url;
    }
    if (name.endsWith("arm64_fixed_webview2-setup.nsis.zip.sig")) {
      const sig = await getSignature(browser_download_url);
      updateData.platforms["windows-aarch64"].signature = sig;
    }
  });

  await Promise.all(promises);

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

  if (!response.ok) {
    throw new Error(
      `Failed to fetch signature ${url}: ${response.status} ${response.statusText}`
    );
  }

  return response.text();
}

resolveUpdater().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
