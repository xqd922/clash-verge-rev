import fse from "fs-extra";
import path from "path";
import { getOctokit, context } from "@actions/github";

async function main() {
  const token = process.env.GITHUB_TOKEN;
  const releaseId = Number(process.env.RELEASE_ID);
  const files = process.argv.slice(2);

  if (!token) throw new Error("GITHUB_TOKEN is required");
  if (!releaseId) throw new Error("RELEASE_ID is required");
  if (files.length === 0) throw new Error("at least one file is required");

  const github = getOctokit(token);
  const options = { owner: context.repo.owner, repo: context.repo.repo };
  const { data: assets } = await github.rest.repos.listReleaseAssets({
    ...options,
    release_id: releaseId,
    per_page: 100,
  });

  for (const file of files) {
    const stat = await fse.stat(file).catch(() => null);
    if (!stat || !stat.isFile()) {
      throw new Error(`release asset not found: ${file}`);
    }

    const name = path.basename(file).replaceAll(" ", ".");
    const existing = assets.find((asset) => asset.name === name);
    if (existing) {
      await github.rest.repos.deleteReleaseAsset({
        ...options,
        asset_id: existing.id,
      });
    }

    console.log(`[INFO]: upload ${name}`);
    await github.rest.repos.uploadReleaseAsset({
      ...options,
      release_id: releaseId,
      name,
      data: await fse.readFile(file),
      headers: {
        "content-type": "application/octet-stream",
        "content-length": stat.size,
      },
    });
  }
}

main().catch((e) => {
  console.error(e.message || e);
  process.exit(1);
});
