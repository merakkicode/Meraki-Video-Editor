# Releasing Meraki

Official repository: https://github.com/merakkicode/Meraki-Video-Editor.

## Distribution policy

Publish one end-user package per platform: **Meraki-Setup.exe** (NSIS, Windows x64) and **Meraki.AppImage** (Linux x64). Sources and SHA256SUMS accompany them. MSI, DEB and Flatpak recipes remain developer options; they are not advertised as additional official release variants. Flathub has not published this application.

## Build pipeline

The CI workflow builds FFmpeg, x264 and x265 from checksum-pinned source archives on Ubuntu 22.04, targeting Linux and Windows (MinGW cross-compilation). It retains the exact source archives, licenses, configure output and build scripts. Windows codec executables use static codec/toolchain linkage and are checked for unexpected toolchain DLL dependencies. The Linux AppImage also retains source packages for the distribution libraries it includes.

Both desktop jobs run metadata validation, rustfmt, Rust tests, Clippy with warnings denied, frontend tests/build, package creation and real H.264/AAC, H.265 and ProRes encode/decode checks. Linux additionally launches the AppImage under a virtual display. Device-dependent playback tests still require the manual checks in [testing.md](testing.md); a startup smoke test is not a listening or GPU performance test.

Developer runtime helpers may use different upstream packages. Official artifacts must come from this release pipeline so their binaries and corresponding-source archives stay paired. Never reuse arbitrary local packages or omit dependency notices.

## Publish a stable version

1. Update Cargo workspace, npm and Tauri versions together; update the UI version, AppStream metadata, changelog, download links and release notes.
2. Push the reviewed changes and verify the CI results. Complete the applicable manual platform checks.
3. Confirm the version with `RELEASE_TAG=v1.0.0 node scripts/check-release.mjs` (set the environment variable using your shell syntax).
4. Create and push the annotated version tag:

```sh
git tag -a v1.0.0 -m 'Meraki Video Editor 1.0.0'
git push origin v1.0.0
```

The tag workflow runs the gated builds, packages source materials and SHA-256 checksums, then publishes the stable release as Latest. No external source-archive repository variable is required. The workflow refuses a tag that differs from the package version. A failed build leaves the tag without a published binary release; repair the build and inspect the commit before retrying. Never silently replace a published version's binaries.

After publication, verify the two download links, asset checksums, release tag and source materials. Keep the source package available for as long as the associated binaries are distributed. Packages are unsigned until signing credentials are configured; do not describe SHA-256 hashes as signatures.

## Initial repository scripts

`scripts/publish.ps1` and `scripts/publish.sh` initialize a new, empty local Git repository only. They refuse an existing repository. They are not the normal update/release procedure for this repository. Configure an appropriate Meraki team commit identity before publishing.

## Existing project data

The application ID is `org.merakkicode.Meraki`. Earlier development builds used `studio.meraki.editor`. Saved `.meraki` documents remain readable; old recovery journals and proxy caches are not silently migrated. Recover/save unfinished work with the earlier build before switching and regenerate proxies as needed.
