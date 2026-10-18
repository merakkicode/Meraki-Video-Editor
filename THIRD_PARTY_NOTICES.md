# Third-party software

Meraki's original code is MIT OR Apache-2.0. This grant does not relicense dependencies or redistribute third-party authorship.

Rust and frontend dependencies are recorded in Cargo.lock and apps/desktop/package-lock.json. Retain their copyright and license notices when distributing compiled packages. Tauri, wgpu, CPAL, SolidJS, fontdue and other dependencies remain the work of their respective contributors.

FFmpeg is a separate command-line runtime, invoked as a subprocess. Builds enabling libx264/libx265 include GPL components; they are not covered by Meraki's dual license. Windows packages retain the upstream runtime LICENSE and README. Linux packaging retains FFmpeg and system-library notices. See https://ffmpeg.org/legal.html and https://www.gyan.dev/ffmpeg/builds/ for upstream licensing/build information.

Before public binary distribution, supply the complete corresponding sources, configuration and notices for the exact runtime and libraries distributed, alongside the release assets. A source URL alone is not a replacement for the required distribution materials. The release workflow requires a third-party-source archive supplied by maintainers and fails if it is absent. This archive must cover both platform runtimes and their bundled dependencies.

Linux titles use DejaVu fonts under their upstream font license; packages retain the distribution's font copyright file. Windows titles use installed system fonts, which are not copied into the package. Do not strip upstream notices or claim third-party components as original Meraki work.
