# Third-party software

Original Meraki code is MIT OR Apache-2.0. This grant does not relicense dependencies or transfer third-party authorship. Rust and frontend dependencies are recorded in Cargo.lock and apps/desktop/package-lock.json. Tauri, wgpu, CPAL, SolidJS, fontdue and other dependencies remain the work of their respective contributors.

## Media runtime

Official packages invoke FFmpeg as a separate command-line process. FFmpeg builds enable GPL components x264 and x265; that runtime is distributed under its applicable GPL terms, not Meraki's dual license. The package preserves FFmpeg, x264 and x265 license texts. Its corresponding source archives, build script, configuration records and toolchain versions are included in the source-materials asset on the same release page as the binaries.

The checked-in release build script records exact input checksums. If runtime versions, options or dependencies change, rebuild and publish the matching source materials alongside those binaries. Upstream information: [FFmpeg licensing](https://ffmpeg.org/legal.html), [x264](https://www.videolan.org/developers/x264.html), [x265](https://www.videolan.org/developers/x265.html).

The Windows development helper downloads a separate upstream runtime from Gyan and retains its LICENSE and README. That helper is for local development; the official release workflow builds its own runtime and does not redistribute that development download.

## Linux libraries and fonts

AppImage includes distribution libraries and DejaVu fonts with their existing licenses. The release retains the corresponding distribution source packages and font notice, identified by package/version. The desktop depends on the operating system's graphics and audio drivers. Windows titles use installed system fonts; those fonts are not copied into the installer.

The source-materials archive is linked from the release together with the binaries and SHA256SUMS. Keep upstream copyright and license notices intact in all redistributions.
