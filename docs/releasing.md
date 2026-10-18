# Releasing Meraki

The public repository is intended for https://github.com/merakkicode/Meraki-Video-Editor. Publication scripts prepare a local repository and optionally push; they do not create a GitHub repository, configure account credentials or overwrite existing history.

## Repository setup

Create an empty repository under merakkicode. Enable GitHub Actions and private vulnerability reporting. Configure branch protection after the first push. Set repository variables THIRD_PARTY_SOURCE_URL and THIRD_PARTY_SOURCE_SHA256 to the HTTPS archive and checksum of complete corresponding sources for the exact bundled Windows/Linux runtimes. The release workflow refuses publication without this material. See THIRD_PARTY_NOTICES.md.

Review the source tree and gitignore before committing. Local media, caches, installers, dependency directories and project recovery files must remain excluded. Use an appropriate team commit email; the script only changes repository-local identity.

```powershell
./scripts/publish.ps1 -Email 'YOUR_TEAM_COMMIT_EMAIL'
git show --stat
git push -u origin main --tags
```

```sh
export MERAKI_COMMIT_EMAIL='YOUR_TEAM_COMMIT_EMAIL'
bash scripts/publish.sh
git show --stat
git push -u origin main --tags
```

Alternatively pass -Push (PowerShell) or --push (Bash) to include the push. Both scripts refuse an existing .git directory. They create main, origin, the initial commit and annotated v1.0.0-rc1 tag. No force push is used.

## Packaging

Windows: scripts/build-windows.ps1 builds NSIS and MSI. Linux: scripts/build-linux.sh builds the pinned FFmpeg runtime, AppImage and Debian package. Platform-specific Tauri configurations keep Windows resources out of Linux bundles. The native package version is 1.0.0; the Git tag and release channel identify rc1.

The tag workflow invokes the same matrix as CI. It publishes only after both platform builds and the Flatpak build/linkage check succeed. It normalizes asset names to Meraki-Setup.exe, Meraki.msi, Meraki.AppImage and meraki.deb, attaches sources and SHA256SUMS, and marks tags containing a hyphen as prereleases. It does not sign packages. Review release-specific source materials whenever runtime versions change.

Do not claim successful Linux execution from Windows compilation. Complete the manual platform acceptance checklist in testing.md. Linux's FFmpeg build includes x264/x265 and packages its dynamic dependency closure; maintain that closure and notices when changing the build baseline.

## Flatpak and Flathub

The checked-in manifest uses the local payload produced by scripts/flatpak-payload.sh. Run it after a Linux release build, then:

```sh
flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.gnome.Platform//49 org.gnome.Sdk//49
flatpak-builder --user --force-clean --repo=packaging/flatpak/repo packaging/flatpak/build packaging/flatpak/org.merakkicode.Meraki.yml
flatpak build-bundle packaging/flatpak/repo Meraki.flatpak org.merakkicode.Meraki
flatpak install --user ./Meraki.flatpak
```

scripts/collect-release.mjs produces a submission manifest with the release payload URL and its real SHA-256, not a placeholder checksum. Submit that resolved manifest and metadata after the release assets exist. Flathub may request a source-build recipe, narrower filesystem permissions, application-ID ownership evidence or additional runtime modules. The requested org.merakkicode.Meraki ID must be accepted by reviewers; GitHub organization ownership alone does not establish ownership of a matching domain.

The manifest grants GPU, Wayland/X11, audio and host-filesystem access for media editing. It does not grant host process execution or unrestricted device access. The ALSA plugin routes audio through the PulseAudio socket, including PipeWire's compatibility service. Validate playback inside the sandbox, not just ELF linkage.

Flathub publication is a separate review process. Until approved, do not advertise the Flathub install command as currently available.

## Existing project data

The application ID changes from studio.meraki.editor to org.merakkicode.Meraki. Saved .meraki documents remain readable. Old recovery journals and proxy caches are not deleted or silently migrated; recover/save unfinished work with the previous build before switching, then open the saved project in the new build. Regenerate preview proxies as needed.

References: [Tauri Windows](https://v2.tauri.app/distribute/windows-installer/), [AppImage](https://v2.tauri.app/distribute/appimage/), [Flatpak](https://v2.tauri.app/distribute/flatpak/), [FFmpeg distribution](https://ffmpeg.org/legal.html).
