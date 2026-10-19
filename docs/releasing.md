# Releasing Meraki

Official repository: https://github.com/merakkicode/Meraki-Video-Editor.

## Distribution policy

Publish one end-user package per platform: **Meraki-Setup.exe** (NSIS, Windows x64) and **Meraki.AppImage** (Linux x64). Sources and SHA256SUMS accompany them. MSI, DEB and Flatpak recipes remain developer options; they are not advertised as additional official release variants. Flathub has not published this application. AppStream catalog metadata is not shipped: its permitted metadata licenses exclude Apache-2.0, the sole license for original project material.

## Build pipeline

The CI workflow builds FFmpeg, x264 and x265 from checksum-pinned source archives on Ubuntu 22.04, targeting Linux and Windows (MinGW cross-compilation). It retains the exact source archives, licenses, configure output and build scripts. Windows codec executables use static codec/toolchain linkage and are checked for unexpected toolchain DLL dependencies. The Linux AppImage also retains source packages for the distribution libraries it includes.

Both desktop jobs run metadata validation, rustfmt, Rust tests, Clippy with warnings denied, frontend tests/build, package creation and real H.264/AAC, H.265 and ProRes encode/decode checks. Linux additionally launches the AppImage under a virtual display. Device-dependent playback tests still require the manual checks in [testing.md](testing.md); a startup smoke test is not a listening or GPU performance test.

Developer runtime helpers may use different upstream packages. Official artifacts must come from this release pipeline so their binaries and corresponding-source archives stay paired. Never reuse arbitrary local packages or omit dependency notices.

## Publish a stable version

1. Update Cargo workspace, npm and Tauri versions together; update the UI version, changelog, download links and release notes.
2. Push the reviewed version/metadata changes to `main`. A change to `Cargo.toml` (or the release workflow) starts the release pipeline. It validates metadata and checks whether that stable version is already published; existing stable releases are skipped on main.
3. The pipeline runs all gated builds. Only after both platforms pass does it create the annotated `v<version>` tag at the exact built commit and publish the release as Latest. Existing tags must resolve to that same commit. Sources, project notices and SHA-256 checksums accompany the binaries.
4. For an unpublished version, maintainers can retry from Actions → Release → Run workflow on `main`. Explicit retries of published versions are rejected. Pushing an annotated version tag remains supported; its name must match the package version.

A failed build leaves the version unpublished. Repair the failure and inspect the commit before retrying. Never silently replace a published version's binaries. Complete the applicable manual platform checks as well; CI smoke checks do not establish device-specific playback or GPU performance.

After publication, verify the two download links, asset checksums, release tag and source materials. Keep the source package available for as long as the associated binaries are distributed. Packages are unsigned until signing credentials are configured; do not describe SHA-256 hashes as signatures.

## Initial repository scripts

`scripts/publish.ps1` and `scripts/publish.sh` initialize a new, empty local Git repository only. They refuse an existing repository. They are not the normal update/release procedure for this repository. Configure an appropriate Meraki team commit identity before publishing.

## Existing project data

The application ID is `org.merakkicode.Meraki`. Earlier development builds used `studio.meraki.editor`. Saved `.meraki` documents remain readable; old recovery journals and proxy caches are not silently migrated. Recover/save unfinished work with the earlier build before switching and regenerate proxies as needed.
