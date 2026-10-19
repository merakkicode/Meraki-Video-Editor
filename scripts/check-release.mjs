import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
const read = path => readFileSync(new URL(`../${path}`, import.meta.url), 'utf8');
const config = JSON.parse(read('apps/desktop/src-tauri/tauri.conf.json'));
const pkg = JSON.parse(read('apps/desktop/package.json'));
assert.equal(config.version, pkg.version, 'Native and frontend versions must agree');
assert.equal(config.productName, 'Meraki Video Editor');
assert.equal(pkg.repository, 'https://github.com/merakkicode/Meraki-Video-Editor');
assert.equal(pkg.license, 'Apache-2.0');
assert.equal(config.bundle.license, 'Apache-2.0');
assert.equal(config.bundle.licenseFile, '../../../LICENSE');
assert.equal(JSON.parse(read('apps/desktop/package-lock.json')).packages[''].license, 'Apache-2.0');
assert.match(read('Cargo.toml'), /^license = "Apache-2.0"$/m);
assert.match(read('LICENSE'), /Apache License\s+Version 2\.0, January 2004/);
assert.ok(!existsSync(new URL('../LICENSE-MIT', import.meta.url)), 'Remove the former project MIT license');
for (const platform of ['windows', 'linux']) {
  const { bundle } = JSON.parse(read(`apps/desktop/src-tauri/tauri.${platform}.conf.json`));
  for (const name of ['LICENSE', 'NOTICE', 'THIRD_PARTY_NOTICES.md']) {
    assert.equal(bundle.resources[`../../../${name}`], `licenses/meraki/${name}`);
  }
}
assert.ok(read('Cargo.toml').includes(`version = "${config.version}"`));
assert.deepEqual(JSON.parse(read('apps/desktop/src-tauri/tauri.windows.conf.json')).bundle.targets, ['nsis']);
assert.deepEqual(JSON.parse(read('apps/desktop/src-tauri/tauri.linux.conf.json')).bundle.targets, ['appimage']);
const tag = process.env.RELEASE_TAG;
if (tag) assert.equal(tag, `v${config.version}`, 'Release tag must match the stable package version');
console.log(`Meraki ${config.version}: release metadata verified`);
