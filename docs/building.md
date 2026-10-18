# Building Meraki

Use stable Rust with rustfmt/Clippy, Node.js 24, npm and Git. Windows additionally requires Visual Studio C++ Build Tools and the Windows SDK. Linux uses Ubuntu 22.04 or newer with WebKitGTK 4.1 development headers.

```sh
git clone https://github.com/merakkicode/Meraki-Video-Editor.git
cd Meraki-Video-Editor
```

## Windows development

```powershell
./scripts/setup-media.ps1
cargo test --workspace --locked
cd apps/desktop
npm ci
npm test
npm run dev
```

The development helper obtains a checksum-verified upstream media runtime. Official release builds use the source-built runtime from CI instead; do not republish development bundles as official release assets.

## Linux development

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config curl file libwebkit2gtk-4.1-dev libgtk-3-dev libasound2-dev libasound2-plugins libssl-dev librsvg2-dev patchelf libvulkan-dev fonts-dejavu-core nasm yasm libx264-dev libx265-dev libfreetype6-dev libharfbuzz-dev libfontconfig1-dev zlib1g-dev
bash scripts/setup-media.sh
cargo test --workspace --locked
cd apps/desktop
npm ci
npm test
npm run dev
```

## Validation

From the repository root:

```sh
node scripts/check-release.mjs
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Then run `npm test` and `npm run build` in `apps/desktop`. See [testing.md](testing.md) for device-dependent integration tests. `npm run dev:web` previews the frontend in a browser; native media and project operations require the desktop application.

## Packages

Run `scripts/build-windows.ps1` or `bash scripts/build-linux.sh` to build locally. Default targets are NSIS and AppImage. The checked-in Debian/WiX settings are developer packaging options, not separate official downloads. Flatpak remains experimental.

Official builds, runtime sources, package checks and publication are defined in [.github/workflows](../.github/workflows). See [Releasing](releasing.md).
