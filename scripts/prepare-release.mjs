import assert from 'node:assert/strict';
import { appendFileSync, readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export async function selectRelease({ version, refType, refName, event, repository, token, request = fetch }) {
  assert.match(version, /^\d+\.\d+\.\d+$/, 'Only stable versions can be published');
  const tag = `v${version}`;
  if (refType === 'tag') assert.equal(refName, tag, 'Release tag must match the package version');
  else assert.equal(refName, 'main', 'Stable releases must come from main or a version tag');
  const response = await request(`https://api.github.com/repos/${repository}/releases/tags/${tag}`, {
    headers: { Accept: 'application/vnd.github+json', Authorization: `Bearer ${token}` },
  });
  if (response.status === 404) return { tag, publish: true };
  if (response.status !== 200) throw new Error(`Release lookup failed: HTTP ${response.status}`);
  const release = await response.json();
  assert.ok(!release.draft && !release.prerelease, 'An existing draft/prerelease requires maintainer review');
  if (event === 'push' && refType === 'branch') return { tag, publish: false };
  throw new Error(`${tag} is already published; never replace published binaries`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { version } = JSON.parse(readFileSync(new URL('../apps/desktop/package.json', import.meta.url), 'utf8'));
  const tag = `v${version}`;
  process.env.RELEASE_TAG = tag;
  await import('./check-release.mjs');
  const result = await selectRelease({
    version, refType: process.env.GITHUB_REF_TYPE, refName: process.env.GITHUB_REF_NAME,
    event: process.env.GITHUB_EVENT_NAME, repository: process.env.GITHUB_REPOSITORY, token: process.env.GH_TOKEN,
  });
  appendFileSync(process.env.GITHUB_OUTPUT, `tag=${result.tag}\npublish=${result.publish}\n`);
  console.log(result.publish ? `Prepare ${result.tag}` : `${result.tag} already published; skipping`);
}
