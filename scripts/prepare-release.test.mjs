import assert from 'node:assert/strict';
import test from 'node:test';
import { selectRelease } from './prepare-release.mjs';

const base = { version: '1.0.0', refType: 'branch', refName: 'main', event: 'push', repository: 'merakkicode/Meraki-Video-Editor', token: 'test' };
const request = (status, release = {}) => async () => ({ status, json: async () => release });
test('a missing stable release is selected, including manual retries', async () => {
  for (const event of ['push', 'workflow_dispatch']) {
    assert.deepEqual(await selectRelease({ ...base, event, request: request(404) }), { tag: 'v1.0.0', publish: true });
  }
});
test('an existing release is skipped on main and rejected for explicit republishing', async () => {
  assert.deepEqual(await selectRelease({ ...base, request: request(200) }), { tag: 'v1.0.0', publish: false });
  for (const extra of [{ event: 'workflow_dispatch' }, { refType: 'tag', refName: 'v1.0.0' }]) {
    await assert.rejects(selectRelease({ ...base, ...extra, request: request(200) }), /already published/);
  }
});
test('drafts, prereleases, wrong tags, branches and unstable versions are rejected', async () => {
  for (const extra of [{ refType: 'tag', refName: 'v2.0.0' }, { refName: 'feature' }, { version: '1.0.0-rc1' }]) {
    await assert.rejects(selectRelease({ ...base, ...extra, request: request(404) }));
  }
  for (const release of [{ draft: true }, { prerelease: true }]) {
    await assert.rejects(selectRelease({ ...base, request: request(200, release) }), /review/);
  }
});
test('API failures fail closed instead of being mistaken for a missing release', async () => {
  for (const status of [401, 403, 429, 500]) {
    await assert.rejects(selectRelease({ ...base, request: request(status) }), /lookup failed/);
  }
  await assert.rejects(selectRelease({ ...base, request: async () => { throw new Error('offline'); } }), /offline/);
});
