import { readdirSync, mkdirSync, copyFileSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'release-assets');
mkdirSync(out, { recursive: true });
function walk(dir) { return readdirSync(dir, { withFileTypes:true }).flatMap(e => e.isDirectory() ? walk(join(dir,e.name)) : [join(dir,e.name)]); }
const wanted = process.platform === 'win32' ? { '.exe':'Meraki-Setup.exe', '.msi':'Meraki.msi' } : { '.AppImage':'Meraki.AppImage', '.deb':'meraki.deb' };
for (const [ext,name] of Object.entries(wanted)) {
  const matches = walk(join(root,'target/release/bundle')).filter(p => p.endsWith(ext));
  if(matches.length !== 1) throw new Error(`Expected exactly one ${ext} bundle, found ${matches.length}`);
  copyFileSync(matches[0],join(out,name));
}
const tag=process.env.GITHUB_REF_NAME || 'v1.0.0-rc1';
if(process.platform === 'linux') {
  const sha=createHash('sha256').update(readFileSync(join(out,'meraki-linux-payload.tar.gz'))).digest('hex');
  const manifest=readFileSync(join(root,'packaging/flatpak/org.merakkicode.Meraki.yml'),'utf8')
    .replace('path: ../../release-assets/meraki-linux-payload.tar.gz', `url: https://github.com/merakkicode/meraki/releases/download/${tag}/meraki-linux-payload.tar.gz\n        sha256: ${sha}\n        only-arches: [x86_64]`);
  writeFileSync(join(out,'org.merakkicode.Meraki.yml'),manifest);
}
const lines=readdirSync(out).filter(n=>!n.endsWith('.sha256')).sort().map(name=>`${createHash('sha256').update(readFileSync(join(out,name))).digest('hex')}  ${name}`);
writeFileSync(join(out,`${process.platform}.sha256`),lines.join('\n')+'\n');
