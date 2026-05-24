const LEGACY_NUMBERED_TAG_RE = /^v(\d+(?:\.\d+)*)-legacy\.(\d+)$/;

export function resolveLegacyReleaseTag(
  tags,
  releaseTag = process.env.RELEASE_TAG
) {
  const explicitTag = normalizeReleaseTag(releaseTag);
  if (explicitTag) {
    return { name: explicitTag };
  }

  const numberedTags = tags
    .map((tag) => ({ ...tag, parsed: parseNumberedLegacyTag(tag.name) }))
    .filter((tag) => tag.parsed);

  numberedTags.sort((a, b) => compareNumberedLegacyTags(b.parsed, a.parsed));

  const latestTag = numberedTags[0];
  if (!latestTag) {
    throw new Error("could not find a numbered legacy release tag");
  }

  return { name: latestTag.name };
}

function normalizeReleaseTag(tag) {
  const value = tag?.trim();
  if (!value) {
    return null;
  }
  return value.startsWith("v") ? value : `v${value}`;
}

function parseNumberedLegacyTag(tag) {
  const match = LEGACY_NUMBERED_TAG_RE.exec(tag);
  if (!match) {
    return null;
  }

  return {
    version: match[1].split(".").map((part) => Number.parseInt(part, 10)),
    legacy: Number.parseInt(match[2], 10),
  };
}

function compareNumberedLegacyTags(left, right) {
  const length = Math.max(left.version.length, right.version.length);
  for (let index = 0; index < length; index += 1) {
    const diff = (left.version[index] || 0) - (right.version[index] || 0);
    if (diff !== 0) {
      return diff;
    }
  }

  return left.legacy - right.legacy;
}
